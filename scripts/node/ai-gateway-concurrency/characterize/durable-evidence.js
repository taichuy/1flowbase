'use strict';

const { performance } = require('node:perf_hooks');
const { TRANSPORT } = require('../contracts');

const DURABLE_GRACE_MS = 10_000;
const DURABLE_POLL_INTERVAL_MS = 100;
const TERMINAL_STATUSES = new Set(['succeeded', 'incomplete', 'failed', 'cancelled']);
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;

function unwrapData(payload) {
  return payload && typeof payload === 'object' && 'data' in payload ? payload.data : payload;
}

function runUuidFromProtocolId(transport, protocolId) {
  const prefix = transport === TRANSPORT.RESPONSES_SSE
    ? 'resp_'
    : transport === TRANSPORT.ANTHROPIC_SSE ? 'msg_' : null;
  if (!prefix || typeof protocolId !== 'string' || !protocolId.startsWith(prefix)) return null;
  const runId = protocolId.slice(prefix.length);
  return UUID_PATTERN.test(runId) ? runId : null;
}

function listItems(payload) {
  const data = unwrapData(payload);
  return Array.isArray(data?.items) ? data.items : [];
}

function activeTotal(payload) {
  const data = unwrapData(payload);
  return Number.isInteger(data?.active?.total) ? data.active.total : null;
}

function activeStreams(payload) {
  const data = unwrapData(payload);
  if (!Array.isArray(data?.streams)) return null;
  return data.streams.map((stream) => ({
    invocationId: stream?.invocation_id ?? null,
    providerCode: stream?.provider_code ?? null,
    protocol: stream?.protocol ?? null,
    transport: stream?.transport ?? null,
    status: stream?.status ?? null,
  }));
}

async function readJson(endpoint, fetchImpl) {
  const response = await fetchImpl(endpoint.url, {
    method: endpoint.method ?? 'GET',
    headers: endpoint.headers ?? {},
  });
  if (!response.ok) throw new Error(`durable evidence endpoint returned HTTP ${response.status}`);
  return response.json();
}

function requestIdentityFailures(requests) {
  const failures = [];
  const seen = new Map();
  for (const request of requests) {
    if (request.protocolIdCount !== 1) {
      failures.push(`${request.clientNonce}: expected one protocol run id, received ${request.protocolIdCount ?? 0}`);
    }
    if (!request.runId) failures.push(`${request.clientNonce}: protocol id did not contain a ${request.transport} run UUID`);
    if (request.runId) {
      const prior = seen.get(request.runId);
      if (prior) failures.push(`${request.clientNonce}: duplicate run UUID also used by ${prior}`);
      else seen.set(request.runId, request.clientNonce);
    }
  }
  return failures;
}

function evaluateSnapshot(requests, snapshot) {
  const failures = requestIdentityFailures(requests);
  for (const request of requests) {
    const matches = snapshot.lists[request.transport]?.filter(
      (item) => item.externalTraceId === request.clientNonce,
    ) ?? [];
    if (matches.length !== 1) {
      failures.push(`${request.clientNonce}: expected one list correlation, received ${matches.length}`);
      continue;
    }
    const listed = matches[0];
    const queried = snapshot.queries[request.clientNonce] ?? null;
    if (listed.id !== request.runId) failures.push(`${request.clientNonce}: protocol/list run id mismatch`);
    if (!TERMINAL_STATUSES.has(listed.status)) failures.push(`${request.clientNonce}: list status remained ${listed.status}`);
    if (!queried) failures.push(`${request.clientNonce}: query_run result missing`);
    else {
      if (queried.id !== request.runId) failures.push(`${request.clientNonce}: protocol/query run id mismatch`);
      if (queried.id !== listed.id) failures.push(`${request.clientNonce}: list/query run id mismatch`);
      if (queried.status !== listed.status) failures.push(`${request.clientNonce}: list/query status mismatch`);
      if (!TERMINAL_STATUSES.has(queried.status)) failures.push(`${request.clientNonce}: query status remained ${queried.status}`);
    }
  }
  for (const [transport, total] of Object.entries(snapshot.runtimeActiveTotals)) {
    if (total !== 0) failures.push(`${transport}: runtime data.active.total was ${total}`);
  }
  for (const [url, streams] of Object.entries(snapshot.pluginStreams)) {
    if (!Array.isArray(streams)) failures.push(`${url}: plugin streams response was invalid`);
    else if (streams.length !== 0) failures.push(`${url}: plugin streams contained ${streams.length} active stream(s)`);
  }
  return failures;
}

function sanitizedListItems(payload) {
  return listItems(payload).map((item) => ({
    id: item?.id ?? null,
    status: item?.status ?? null,
    externalTraceId: item?.correlation?.external_trace_id ?? null,
  }));
}

function sanitizedQuery(payload) {
  const data = unwrapData(payload);
  return { id: data?.id ?? null, status: data?.status ?? null };
}

async function captureSnapshot(requests, targetsByTransport, fetchImpl) {
  const lists = {};
  const queries = {};
  const runtimeActiveTotals = {};
  const pluginStreams = {};
  await Promise.all(Object.entries(targetsByTransport).map(async ([transport, target]) => {
    lists[transport] = sanitizedListItems(await readJson(target.durable.list_runs, fetchImpl));
    runtimeActiveTotals[transport] = activeTotal(await readJson(target.runtime_activity, fetchImpl));
  }));
  await Promise.all(requests.filter((request) => request.runId).map(async (request) => {
    const template = targetsByTransport[request.transport].durable.query_run;
    const endpoint = { ...template, url: template.url_template.replace('{run_id}', request.runId) };
    queries[request.clientNonce] = sanitizedQuery(await readJson(endpoint, fetchImpl));
  }));
  const uniqueStreamEndpoints = new Map(Object.values(targetsByTransport).map((target) => [
    target.plugin_runner_active_streams.url,
    target.plugin_runner_active_streams,
  ]));
  await Promise.all([...uniqueStreamEndpoints].map(async ([url, endpoint]) => {
    pluginStreams[url] = activeStreams(await readJson(endpoint, fetchImpl));
  }));
  return { lists, queries, runtimeActiveTotals, pluginStreams };
}

async function collectDurableConvergence({
  requestEvents,
  targetsByTransport,
  fetchImpl = globalThis.fetch,
  graceMs = DURABLE_GRACE_MS,
  pollIntervalMs = DURABLE_POLL_INTERVAL_MS,
}) {
  const startedAt = new Date().toISOString();
  const started = performance.now();
  const requests = requestEvents
    .filter((event) => [TRANSPORT.RESPONSES_SSE, TRANSPORT.ANTHROPIC_SSE].includes(event.transport))
    .map((event) => ({
      clientNonce: event.clientNonce,
      transport: event.transport,
      protocolId: event.protocolId ?? null,
      protocolIdCount: event.protocolIdCount ?? 0,
      runId: runUuidFromProtocolId(event.transport, event.protocolId),
    }));
  const polls = [];
  let failures = requestIdentityFailures(requests);
  while (performance.now() - started <= graceMs) {
    try {
      const snapshot = await captureSnapshot(requests, targetsByTransport, fetchImpl);
      failures = evaluateSnapshot(requests, snapshot);
      polls.push({ offsetMs: Math.round(performance.now() - started), ...snapshot, failures });
      if (failures.length === 0) break;
    } catch (error) {
      failures = [`poll failed: ${error.message}`];
      polls.push({ offsetMs: Math.round(performance.now() - started), error: error.message, failures });
    }
    if (performance.now() - started + pollIntervalMs > graceMs) break;
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  }
  return {
    schemaVersion: 1,
    verdict: failures.length === 0 ? 'PASS' : 'FAIL',
    graceMs,
    pollIntervalMs,
    startedAt,
    completedAt: new Date().toISOString(),
    requests,
    polls,
    failures,
  };
}

module.exports = {
  DURABLE_GRACE_MS,
  DURABLE_POLL_INTERVAL_MS,
  collectDurableConvergence,
  evaluateSnapshot,
  runUuidFromProtocolId,
};
