'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const {
  MOCK_ROUTE,
  SCENARIO,
  TRANSPORT,
  assertDistinctRequestNonces,
  mockScenarioSentinel,
} = require('../../contracts');
const { createMockUpstream, wireAuditVectorFromBody } = require('..');
const { DEFAULT_BARRIER_MARKERS } = require('../protocol-events');

async function withMockUpstream(run, options = {}) {
  const upstream = createMockUpstream({
    slowChunkDelayMs: 15,
    cancelObservationMs: 150,
    ...options,
  });
  const endpoints = await upstream.start();
  try {
    await run({ upstream, ...endpoints });
  } finally {
    await upstream.stop();
  }
}

function parseSse(text) {
  return text.trim().split('\n\n').map((block) => {
    const lines = block.split('\n');
    const event = lines.find((line) => line.startsWith('event: '))?.slice(7);
    const data = lines.find((line) => line.startsWith('data: '))?.slice(6);
    return { event, data: JSON.parse(data) };
  });
}

test('wire audit vector is inferred from the public Responses body', () => {
  assert.equal(wireAuditVectorFromBody({
    tools: [{ type: 'tool_search' }],
    input: [{ type: 'tool_search_call' }],
  }), 'tool-search-additional-tools');
  assert.equal(wireAuditVectorFromBody({
    input: [{ type: 'tool_search_output' }, { type: 'additional_tools' }],
  }), 'tool-search-output-additional-tools');
  assert.equal(wireAuditVectorFromBody({
    tools: [{ type: 'file_search' }, { type: 'programmatic_tool_calling' }],
  }), 'hosted-tools');
  assert.equal(wireAuditVectorFromBody({ tools: [{ type: 'mcp' }] }), 'mcp-list-call-approval');
  assert.equal(wireAuditVectorFromBody({
    previous_response_id: 'resp_previous',
    input: [{ type: 'mcp_approval_response', approval_request_id: 'approval_1', approve: true }],
  }), 'mcp-approval-continuation');
  assert.equal(wireAuditVectorFromBody({ input: 'ordinary request' }), null);
});

async function waitFor(predicate, timeoutMs = 500) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = predicate();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error('timed out waiting for mock evidence');
}

async function readStreamChunk(reader, timeoutMs = 250) {
  let timer;
  try {
    return await Promise.race([
      reader.read(),
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error('marker-1 item did not become observable before the barrier')),
          timeoutMs,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function arrivalEntries(upstream) {
  return upstream.snapshot().entries.filter((entry) => entry.event === 'arrival');
}

test('AC-002/003: Responses SSE emits unique nonce, fixed chunks, and exactly one terminal', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl }) => {
    const request = () => fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST',
      headers: {
        accept: 'text/event-stream',
        authorization: 'Bearer must-not-be-recorded',
        'content-type': 'application/json',
        'x-api-key': 'must-not-be-recorded',
      },
      body: JSON.stringify({ model: 'mock-model', stream: true, input: 'private prompt' }),
    });
    const responses = await Promise.all([request(), request()]);
    const streams = await Promise.all(responses.map(async (response) => parseSse(await response.text())));
    const arrivals = arrivalEntries(upstream);
    assert.equal(arrivals.length, 2);
    assertDistinctRequestNonces(arrivals.map((entry) => entry.nonce));
    for (const [index, events] of streams.entries()) {
      assert.deepEqual(events.map((event) => event.event), [
        'response.created',
        'response.output_item.added',
        'response.content_part.added',
        'response.output_text.delta',
        'response.output_text.delta',
        'response.output_text.done',
        'response.content_part.done',
        'response.output_item.done',
        'response.completed',
      ]);
      const deltas = events.filter((event) => event.event === 'response.output_text.delta');
      assert.deepEqual(deltas.map((event) => event.data.delta), [
        `${arrivals[index].nonce}:chunk-1`,
        `${arrivals[index].nonce}:chunk-2`,
      ]);
      assert.equal(events.filter((event) => event.event === 'response.completed').length, 1);
    }
    assert.equal(upstream.snapshot().peak >= 1, true);
    assert.equal(upstream.snapshot().active, 0);
    assert.deepEqual(arrivals[0].request.body, {
      inputCharacters: 14,
      keys: ['input', 'model', 'stream'],
      model: 'mock-model',
      stream: true,
    });
    assert.equal(JSON.stringify(arrivals[0].request).includes('must-not-be-recorded'), false);
    assert.equal(JSON.stringify(arrivals[0].request).includes('private prompt'), false);
  });
});

test('AC-002: Anthropic Messages SSE emits fixed content and one message_stop terminal', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl }) => {
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.ANTHROPIC_MESSAGES}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ model: 'mock-model', stream: true, messages: [{ role: 'user', content: 'secret' }] }),
    });
    const events = parseSse(await response.text());
    const nonce = arrivalEntries(upstream)[0].nonce;
    assert.deepEqual(events.map((event) => event.event), [
      'message_start',
      'content_block_start',
      'content_block_delta',
      'content_block_delta',
      'content_block_stop',
      'message_delta',
      'message_stop',
    ]);
    assert.deepEqual(
      events.filter((event) => event.event === 'content_block_delta').map((event) => event.data.delta.text),
      [`${nonce}:chunk-1`, `${nonce}:chunk-2`],
    );
    assert.equal(events.filter((event) => event.event === 'message_stop').length, 1);
  });
});

test('Root #1440 AC-003: producer barrier releases chunk-2 only after chunk-1 is observable', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl, barrierReleaseUrl }) => {
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ model: 'mock-model', stream: true, input: [] }),
    });
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let visible = '';
    while (!visible.includes(DEFAULT_BARRIER_MARKERS.first)) {
      const { done, value } = await reader.read();
      assert.equal(done, false);
      visible += decoder.decode(value, { stream: true });
    }
    await upstream.waitForEvent('barrier_waiting');
    assert.equal(visible.includes(DEFAULT_BARRIER_MARKERS.second), false);
    assert.doesNotMatch(visible, /response.completed/u);

    const release = await fetch(barrierReleaseUrl, { method: 'POST' });
    assert.equal(release.status, 200);
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      visible += decoder.decode(value, { stream: true });
    }
    assert.match(visible, new RegExp(DEFAULT_BARRIER_MARKERS.second, 'u'));
    assert.match(visible, /response.completed/u);
  }, { barrierEnabled: true });
});

test('AC-002: Responses WebSocket emits fixed nonce chunks and one completed terminal', async () => {
  await withMockUpstream(async ({ upstream, websocketBaseUrl }) => {
    const events = await new Promise((resolve, reject) => {
      const received = [];
      const socket = new WebSocket(`${websocketBaseUrl}${MOCK_ROUTE.RESPONSES}`);
      socket.addEventListener('open', () => socket.send(JSON.stringify({
        type: 'response.create',
        response: { model: 'mock-model', input: 'private prompt' },
      })));
      socket.addEventListener('message', (message) => received.push(JSON.parse(message.data)));
      socket.addEventListener('close', () => resolve(received));
      socket.addEventListener('error', reject);
    });
    const nonce = arrivalEntries(upstream)[0].nonce;
    assert.deepEqual(events.map((event) => event.type), [
      'response.created',
      'response.output_item.added',
      'response.content_part.added',
      'response.output_text.delta',
      'response.output_text.delta',
      'response.output_text.done',
      'response.content_part.done',
      'response.output_item.done',
      'response.completed',
    ]);
    assert.deepEqual(events.filter((event) => event.delta).map((event) => event.delta), [
      `${nonce}:chunk-1`,
      `${nonce}:chunk-2`,
    ]);
  });
});

test('AC-004/005: slow streams overlap and retain arrival/active/peak evidence', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl }) => {
    const request = () => fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
      },
      body: JSON.stringify({ model: 'mock-model', stream: true, input: mockScenarioSentinel(SCENARIO.SLOW) }),
    }).then((response) => response.text());
    await Promise.all([request(), request(), request(), request()]);
    const snapshot = upstream.snapshot();
    assert.equal(snapshot.arrivals, 4);
    assert.equal(snapshot.peak, 4);
    assert.equal(snapshot.active, 0);
    assert.equal(snapshot.entries.filter((entry) => entry.event === 'settled').length, 4);
  });
});

test('AC-004: HTTP 500 is explicit and produces no success terminal', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl }) => {
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.ANTHROPIC_MESSAGES}`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        model: 'mock-model', stream: true,
        messages: [{ role: 'user', content: mockScenarioSentinel(SCENARIO.HTTP_500) }],
      }),
    });
    assert.equal(response.status, 500);
    const evidence = await waitFor(() => upstream.snapshot().entries.find((entry) => entry.outcome === 'http-500'));
    assert.equal(evidence.successTerminalCount, 0);
  });
});

test('AC-004 controlled negative: ambiguous scenario sentinels fail closed', async () => {
  await withMockUpstream(async ({ httpBaseUrl }) => {
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        model: 'mock-model',
        input: `${mockScenarioSentinel(SCENARIO.HTTP_500)} ${mockScenarioSentinel(SCENARIO.SLOW)}`,
      }),
    });
    assert.equal(response.status, 400);
    const payload = await response.json();
    assert.match(payload.error.message, /multiple scenario sentinels/u);
  });
});

test('AC-004: stream interruption closes Responses SSE without a success terminal', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl }) => {
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        model: 'mock-model', stream: true,
        input: mockScenarioSentinel(SCENARIO.STREAM_INTERRUPTION),
      }),
    });
    await assert.rejects(response.text());
    const evidence = await waitFor(() => upstream.snapshot().entries.find((entry) => entry.outcome === 'interrupted'));
    assert.equal(evidence.successTerminalCount, undefined);
    assert.equal(upstream.snapshot().entries.some((entry) => entry.protocolEvent === 'response.completed'), false);
  });
});

test('AC-004: HTTP cancellation is observed and never produces a success terminal', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl }) => {
    const controller = new AbortController();
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        model: 'mock-model', stream: true,
        input: mockScenarioSentinel(SCENARIO.CANCEL_OBSERVATION),
      }),
      signal: controller.signal,
    });
    const reader = response.body.getReader();
    await reader.read();
    controller.abort();
    await assert.rejects(reader.read());
    const evidence = await waitFor(() => upstream.snapshot().entries.find((entry) => entry.outcome === 'cancelled'));
    assert.equal(evidence.successTerminalCount, 0);
  });
});

test('AC-004: WebSocket response.cancel produces cancelled, never completed', async () => {
  await withMockUpstream(async ({ upstream, websocketBaseUrl }) => {
    const events = await new Promise((resolve, reject) => {
      const received = [];
      const socket = new WebSocket(`${websocketBaseUrl}${MOCK_ROUTE.RESPONSES}`);
      socket.addEventListener('open', () => socket.send(JSON.stringify({
        type: 'response.create',
        response: { input: mockScenarioSentinel(SCENARIO.CANCEL_OBSERVATION) },
      })));
      socket.addEventListener('message', (message) => {
        const event = JSON.parse(message.data);
        received.push(event);
        if (event.type === 'response.output_text.delta') socket.send(JSON.stringify({ type: 'response.cancel' }));
      });
      socket.addEventListener('close', () => resolve(received));
      socket.addEventListener('error', reject);
    });
    assert.equal(events.some((event) => event.type === 'response.cancelled'), true);
    assert.equal(events.some((event) => event.type === 'response.completed'), false);
    const evidence = upstream.snapshot().entries.find((entry) => entry.outcome === 'cancelled');
    assert.equal(evidence.successTerminalCount, 0);
  });
});

test('AC-003 controlled negative: reused nonce is rejected as cross-stream evidence', () => {
  assert.throws(
    () => assertDistinctRequestNonces(['mock-000001', 'mock-000001']),
    /mock request nonce was reused/u,
  );
});

test('AC-007: timeline snapshots are detached from internal evidence', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl }) => {
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ model: 'mock-model', stream: true, input: [] }),
    });
    await response.text();
    const snapshot = upstream.snapshot();
    snapshot.entries[0].request.method = 'MUTATED';
    assert.equal(upstream.snapshot().entries[0].request.method, 'POST');
  });
});

// D3-AC-002/003: live producer events, not prepared chronology files, drive the tool barrier.
test('controlled tool loop records live call and second request before barrier release', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl, barrierReleaseUrl }) => {
    const first = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        model: 'mock-model', stream: true,
        input: '1flowbase-client-tool-vector TOOL_VECTOR_PATH=/tmp/tool-vector.txt',
      }),
    });
    const firstEvents = parseSse(await first.text());
    const toolItem = firstEvents.find(
      (event) => event.event === 'response.output_item.done',
    )?.data.item;
    assert.equal(toolItem?.type, 'function_call');
    assert.equal(toolItem?.name, 'shell_command');
    assert.deepEqual(JSON.parse(toolItem?.arguments), {
      command: "cat -- '/tmp/tool-vector.txt'",
      workdir: '/tmp',
    });

    const second = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        model: 'mock-model', stream: true,
        input: [{ type: 'function_call_output', call_id: 'call_fixture', output: '1flowbase-client-tool-result' }],
      }),
    });
    const reader = second.body.getReader();
    const decoder = new TextDecoder();
    let visible = '';
    while (!visible.includes('response.output_item.done') || !visible.includes('marker-1')) {
      const { value } = await readStreamChunk(reader);
      visible += decoder.decode(value, { stream: true });
    }
    assert.doesNotMatch(visible, /marker-2/u);
    await fetch(barrierReleaseUrl, { method: 'POST' });
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      visible += decoder.decode(value, { stream: true });
    }
    assert.match(visible, /marker-2/u);
    const events = upstream.snapshot().entries;
    const call = events.find((event) => event.event === 'tool_call');
    const continuation = events.find((event) => event.event === 'second_upstream_request');
    const released = events.find((event) => event.event === 'barrier_released');
    assert.ok(BigInt(call.monotonic_ns) < BigInt(continuation.monotonic_ns));
    assert.ok(BigInt(continuation.monotonic_ns) < BigInt(released.monotonic_ns));
  }, { barrierEnabled: true });
});

test('Responses tool fixture calls a client-declared read function', async () => {
  await withMockUpstream(async ({ httpBaseUrl }) => {
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        model: 'mock-model', stream: true,
        input: '1flowbase-client-tool-vector TOOL_VECTOR_PATH=/tmp/tool-vector.txt',
        tools: [{
          type: 'function', name: 'read',
          parameters: { type: 'object', properties: { filePath: { type: 'string' } } },
        }],
      }),
    });
    const events = parseSse(await response.text());
    const toolItem = events.find(
      (event) => event.event === 'response.output_item.done',
    )?.data.item;
    assert.equal(toolItem?.name, 'read');
    assert.deepEqual(JSON.parse(toolItem?.arguments), { filePath: '/tmp/tool-vector.txt' });
  });
});

// Root AC-019/020/023/024: provider output, rather than forged client input, drives MCP observations.
test('controlled wire vectors observe honest provider MCP output without executor or server_url outbound', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl, networkObserverUrl, gatewayExecutorObserverUrl }) => {
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        model: 'mock-model', stream: true,
        tools: [
          { type: 'file_search' },
          { type: 'mcp', server_label: 'fixture_mcp', server_url: networkObserverUrl },
          { type: 'custom', name: 'caller', x_gateway_executor_observer: gatewayExecutorObserverUrl },
        ],
        input: 'ordinary user request for an MCP lookup',
      }),
    });
    const events = parseSse(await response.text());
    assert.equal(events.some((event) => event.event === 'response.future_gateway_drift'), false);
    const output = events
      .filter((event) => event.event === 'response.output_item.done')
      .map((event) => event.data.item);
    assert.deepEqual(output.map((item) => item.type), [
      'mcp_list_tools', 'mcp_call', 'mcp_approval_request',
    ]);
    assert.deepEqual(output.map((item) => item.server_label), [
      'fixture_mcp', 'fixture_mcp', 'fixture_mcp',
    ]);
    assert.equal(output.every((item) => typeof item.id === 'string' && item.id.length > 0), true);
    assert.equal(output.every((item) => typeof item.status === 'string' && item.status.length > 0), true);
    assert.equal(output[1].name, 'lookup');
    assert.deepEqual(JSON.parse(output[1].arguments), { query: 'fixture' });
    assert.equal(output[2].name, 'lookup');
    assert.deepEqual(JSON.parse(output[2].arguments), { query: 'approval fixture' });
    const snapshot = upstream.snapshot();
    assert.equal(snapshot.counters.gatewayExecutorInvocations, 0);
    assert.equal(snapshot.counters.networkObserverOutbound, 0);
    assert.equal(snapshot.counters.providerExecutions, 2);
    for (const event of ['mcp_server_definition', 'mcp_list', 'mcp_call', 'mcp_approval']) {
      assert.equal(snapshot.entries.some((entry) => entry.event === event), true);
    }
  });
});
