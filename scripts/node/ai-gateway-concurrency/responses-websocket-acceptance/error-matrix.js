'use strict';

const crypto = require('node:crypto');
const { ERROR_SURFACES, UPSTREAM_ERROR_FIXTURES, errorFixtureMarker, assertUpstreamErrorFidelity } = require('../protocol-oracle/error-fidelity');
const { decodeSseChunks, decodeWebSocketFrames } = require('../protocol-oracle/decoder');
const { createGatewayTarget } = require('./target');
const { TERMINAL_STATUSES } = require('./durable');

function unwrap(payload) { return payload?.data ?? payload; }
async function readJson(url, headers) {
  const response = await fetch(url, { headers, signal: AbortSignal.timeout(10_000) });
  if (!response.ok) throw new Error(`error evidence query returned HTTP ${response.status}`);
  return unwrap(await response.json());
}

function inspectClient(surface, records, success) {
  const events = records.map((record) => record.data).filter(Boolean);
  const messages = events.map((event) => event.response?.error?.message ?? event.error?.message).filter((message) => typeof message === 'string');
  const successCount = surface.startsWith('responses-')
    ? events.filter((event) => event.type === 'response.completed').length
    : surface === 'anthropic-sse'
      ? events.filter((event) => event.type === 'message_stop').length
      : records.filter((record) => record.done).length;
  const failureCount = events.filter((event) => event.type === 'response.failed' || event.type === 'error' || (event.error && !event.type)).length;
  if (success ? (successCount !== 1 || failureCount !== 0 || messages.length !== 0) : (successCount !== 0 || failureCount !== 1 || messages.length !== 1)) {
    throw new Error(`${surface}: terminal/error cardinality mismatch (${successCount} success, ${failureCount} failure, ${messages.length} messages)`);
  }
  return { messages, success_count: successCount, failure_count: failureCount, event_types: events.map((event) => event.type ?? event.object ?? 'error') };
}

async function observeClient(surface, target, fixture, traceId, retryKey) {
  const input = errorFixtureMarker(fixture.id);
  const common = { model: target.model, stream: true, metadata: { trace_id: traceId }, fixture_retry_key: retryKey };
  if (surface === 'responses-websocket') {
    const { collectGatewayFrames } = require('../workflow-contract/gateway-websocket');
    const frames = await collectGatewayFrames(createGatewayTarget({ targets: { openai: target } }), traceId, {
      inputText: input, requestFields: { fixture_retry_key: retryKey },
    });
    return { http_status: 101, records: decodeWebSocketFrames(frames) };
  }
  const url = surface === 'openai-chat-sse' ? target.gateway.chat_completions_url
    : surface === 'anthropic-sse' ? target.gateway.anthropic_messages_url : target.gateway.responses_url;
  const body = surface === 'responses-sse' ? { ...common, input }
    : { ...common, max_tokens: 32, messages: [{ role: 'user', content: input }] };
  const response = await fetch(url, {
    method: 'POST', headers: { authorization: `Bearer ${target.api_key}`, 'content-type': 'application/json', accept: 'text/event-stream', 'anthropic-version': '2023-06-01' },
    body: JSON.stringify(body), signal: AbortSignal.timeout(20_000),
  });
  const raw = await response.text();
  const records = response.headers.get('content-type')?.includes('text/event-stream')
    ? decodeSseChunks([Buffer.from(raw)]) : [{ data: JSON.parse(raw), done: false }];
  return { http_status: response.status, records };
}

async function observeRun(target, traceId) {
  const deadline = Date.now() + 10_000;
  while (true) {
    const listing = await readJson(target.durable.list_runs.url, target.durable.list_runs.headers);
    const matches = (listing.items ?? []).filter((run) => run.correlation?.external_trace_id === traceId);
    if (matches.length > 1) throw new Error('error matrix duplicated durable trace correlation');
    if (matches.length === 1) {
      const runId = matches[0].id;
      const nativeUrl = target.durable.query_run.url_template.replace('{run_id}', encodeURIComponent(runId));
      const native = await readJson(nativeUrl, target.durable.query_run.headers);
      if (TERMINAL_STATUSES.has(native.status)) {
        const overviewUrl = new URL(target.durable.list_runs.url);
        overviewUrl.search = '';
        overviewUrl.pathname += `/${encodeURIComponent(runId)}/overview`;
        const overview = await readJson(overviewUrl.href, target.durable.list_runs.headers);
        if (native.metadata?.external_trace_id !== traceId) throw new Error('Native trace correlation mismatch');
        if (native.id !== runId || overview.flow_run?.id !== runId || overview.flow_run.status !== native.status) throw new Error('Native/durable identity or terminal mismatch');
        return {
          run_id: runId, trace_id: traceId,
          native: { id: native.id, status: native.status, error: native.error ?? null },
          durable: { id: overview.flow_run.id, status: overview.flow_run.status, error_payload: overview.flow_run.error_payload ?? null },
        };
      }
    }
    if (Date.now() >= deadline) throw new Error(`error matrix durable trace ${traceId} did not converge`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

async function runGatewayErrorMatrix({ ready, mockSnapshot }, dependencies = {}) {
  const client = dependencies.observeClient ?? observeClient;
  const run = dependencies.observeRun ?? observeRun;
  const rows = [];
  for (const fixture of UPSTREAM_ERROR_FIXTURES) {
    for (const surface of ERROR_SURFACES) {
      const row = { id: `${fixture.id}/${surface}`, fixture: fixture.id, surface, verdict: 'FAIL', attempts: [] };
      try {
        const target = ready.targets[surface === 'openai-chat-sse' ? 'openai_compatible' : surface === 'anthropic-sse' ? 'anthropic' : 'openai'];
        const retryKey = crypto.randomUUID();
        for (let attempt = 1; attempt <= fixture.attempts; attempt += 1) {
          const traceId = `error-matrix-${crypto.randomUUID()}`;
          const before = mockSnapshot().entries.at(-1)?.sequence ?? 0;
          const observed = await client(surface, target, fixture, traceId, retryKey);
          const success = fixture.id === 'retry' && attempt === 2;
          const projection = inspectClient(surface, observed.records, success);
          const persisted = await run(target, traceId);
          const upstream = mockSnapshot().entries.filter((entry) => entry.sequence > before);
          const arrivals = upstream.filter((entry) => entry.event === 'arrival');
          row.attempts.push({ attempt, http_status: observed.http_status, client: projection, ...persisted, upstream_nonce: arrivals[0]?.nonce ?? null });
          if (arrivals.length !== 1) throw new Error('error matrix attempt must reach exactly one controlled upstream request');
          if (persisted.native.status !== (success ? 'succeeded' : 'failed')) throw new Error('error matrix wrong durable outcome');
          if (!success) {
            if (!upstream.some((entry) => entry.errorFixture === fixture.id && entry.status === fixture.status)) throw new Error('error matrix omitted upstream fixture failure');
            assertUpstreamErrorFidelity(fixture, {
              nativeMessage: persisted.native.error?.message,
              durableMessage: persisted.durable.error_payload?.message,
              clientMessages: projection.messages,
            });
          }
          if (success && (persisted.native.error !== null || persisted.durable.error_payload !== null)) throw new Error('retry success retained an error');
        }
        if (fixture.id === 'retry' && row.attempts[0].run_id === row.attempts[1].run_id) throw new Error('retry reused a durable run');
        row.verdict = 'PASS';
      } catch (error) { row.error = error.message; }
      rows.push(row);
    }
  }
  return { schema_version: '1flowbase.gateway-live-error-matrix/v1', verdict: rows.every((row) => row.verdict === 'PASS') ? 'PASS' : 'FAIL', rows };
}

module.exports = { inspectClient, observeClient, observeRun, runGatewayErrorMatrix };
