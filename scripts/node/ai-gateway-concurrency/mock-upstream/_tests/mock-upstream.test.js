'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const {
  MOCK_ROUTE,
  MOCK_SCENARIO_HEADER,
  SCENARIO,
  TRANSPORT,
  assertDistinctRequestNonces,
} = require('../../contracts');
const { createMockUpstream } = require('..');

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

async function waitFor(predicate, timeoutMs = 500) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = predicate();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error('timed out waiting for mock evidence');
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
        'response.output_text.delta',
        'response.output_text.delta',
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
      'response.output_text.delta',
      'response.output_text.delta',
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
        [MOCK_SCENARIO_HEADER]: SCENARIO.SLOW,
      },
      body: JSON.stringify({ model: 'mock-model', stream: true, input: [] }),
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
        [MOCK_SCENARIO_HEADER]: SCENARIO.HTTP_500,
      },
      body: JSON.stringify({ model: 'mock-model', stream: true, messages: [] }),
    });
    assert.equal(response.status, 500);
    const evidence = await waitFor(() => upstream.snapshot().entries.find((entry) => entry.outcome === 'http-500'));
    assert.equal(evidence.successTerminalCount, 0);
  });
});

test('AC-004: stream interruption closes Responses SSE without a success terminal', async () => {
  await withMockUpstream(async ({ upstream, httpBaseUrl }) => {
    const response = await fetch(`${httpBaseUrl}${MOCK_ROUTE.RESPONSES}`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        [MOCK_SCENARIO_HEADER]: SCENARIO.STREAM_INTERRUPTION,
      },
      body: JSON.stringify({ model: 'mock-model', stream: true, input: [] }),
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
        [MOCK_SCENARIO_HEADER]: SCENARIO.CANCEL_OBSERVATION,
      },
      body: JSON.stringify({ model: 'mock-model', stream: true, input: [] }),
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
      const socket = new WebSocket(`${websocketBaseUrl}${MOCK_ROUTE.RESPONSES}?scenario=${SCENARIO.CANCEL_OBSERVATION}`);
      socket.addEventListener('open', () => socket.send(JSON.stringify({ type: 'response.create', response: {} })));
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
