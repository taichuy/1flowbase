'use strict';

const TERMINAL_STATUSES = new Set(['succeeded', 'incomplete', 'failed', 'cancelled']);

function unwrapData(payload) {
  return payload && typeof payload === 'object' && 'data' in payload ? payload.data : payload;
}

async function readJson(endpoint, fetchImpl = globalThis.fetch) {
  if (!endpoint?.url) throw new Error('local client evidence endpoint URL is required');
  const response = await fetchImpl(endpoint.url, {
    method: endpoint.method ?? 'GET',
    headers: endpoint.headers ?? {},
  });
  if (!response.ok) throw new Error(`local client evidence endpoint returned HTTP ${response.status}`);
  return unwrapData(await response.json());
}

function sanitizeRun(run) {
  return { id: run?.id ?? null, status: run?.status ?? null };
}

async function snapshotRuns(target, fetchImpl = globalThis.fetch) {
  const payload = await readJson(target?.durable?.list_runs, fetchImpl);
  const runs = (Array.isArray(payload?.items) ? payload.items : []).map(sanitizeRun);
  return { ids: runs.map((run) => run.id).filter(Boolean), runs };
}

async function queryRun(target, runId, fetchImpl) {
  const template = target?.durable?.query_run;
  const url = template?.url_template?.replace('{run_id}', encodeURIComponent(runId));
  if (!url || url === template.url_template) throw new Error('durable query endpoint omitted {run_id}');
  return sanitizeRun(await readJson({ ...template, url }, fetchImpl));
}

async function reconcileAttempt({
  target,
  before,
  expectedRuns,
  fetchImpl = globalThis.fetch,
  graceMs = 10_000,
  pollIntervalMs = 100,
}) {
  const known = new Set(before?.ids ?? []);
  const deadline = Date.now() + graceMs;
  let observed = [];
  do {
    const current = await snapshotRuns(target, fetchImpl);
    observed = current.runs.filter((run) => !known.has(run.id));
    if (observed.length > expectedRuns) {
      throw new Error(`expected exactly ${expectedRuns} new durable run, observed ${observed.length}`);
    }
    if (observed.length === expectedRuns
      && observed.every((run) => TERMINAL_STATUSES.has(run.status))) break;
    if (Date.now() >= deadline) break;
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  } while (true);
  if (observed.length !== expectedRuns) {
    throw new Error(`expected exactly ${expectedRuns} new durable run, observed ${observed.length}`);
  }
  if (observed.some((run) => !TERMINAL_STATUSES.has(run.status))) {
    throw new Error('one or more local client durable runs did not reach terminal status');
  }
  const queried = await Promise.all(observed.map((run) => queryRun(target, run.id, fetchImpl)));
  for (const [index, run] of queried.entries()) {
    if (run.id !== observed[index].id || !TERMINAL_STATUSES.has(run.status)) {
      throw new Error(`durable query mismatch for run ${observed[index].id}`);
    }
  }
  return { expected_runs: expectedRuns, runs: queried };
}

function evaluateMockAttempt(before, after, expectedRuns) {
  const cursor = before?.entries?.at(-1)?.sequence ?? 0;
  const events = (after?.entries ?? []).filter((event) => event.sequence > cursor);
  const arrivals = events.filter((event) => event.event === 'arrival');
  const settled = events.filter((event) => event.event === 'settled');
  if (arrivals.length !== expectedRuns) {
    throw new Error(`expected ${expectedRuns} mock arrival, observed ${arrivals.length}`);
  }
  if (settled.length !== expectedRuns || settled.some((event) => event.outcome !== 'completed')) {
    throw new Error(`expected ${expectedRuns} completed mock request, observed ${settled.length}`);
  }
  if (expectedRuns === 2) {
    const tool = events.findIndex((event) => event.event === 'tool_call');
    const second = events.findIndex((event) => event.event === 'second_upstream_request');
    if (tool === -1 || second <= tool) throw new Error('mock tool two-turn chronology was not observed');
  }
  return {
    arrivals: arrivals.length,
    settled: settled.length,
    nonces: arrivals.map((event) => event.nonce),
  };
}

async function waitForBarrierWaiting({
  before,
  mockSnapshot,
  signal,
  graceMs = 180_000,
  pollIntervalMs = 100,
}) {
  if (typeof mockSnapshot !== 'function') throw new Error('mock snapshot reader is required');
  const cursor = before?.entries?.at(-1)?.sequence ?? 0;
  const deadline = Date.now() + graceMs;
  do {
    if (signal?.aborted) throw new Error('client execution ended before mock barrier_waiting was observed');
    const current = await mockSnapshot();
    const waiting = (current?.entries ?? []).find((entry) => (
      entry.sequence > cursor && entry.event === 'barrier_waiting'
    ));
    if (waiting) return waiting;
    if (Date.now() >= deadline) break;
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  } while (true);
  throw new Error('mock barrier_waiting was not observed after the attempt snapshot cursor');
}

async function verifyIdle(
  targets,
  fetchImpl = globalThis.fetch,
  { graceMs = 10_000, pollIntervalMs = 100 } = {},
) {
  const runtimeEndpoints = new Map(targets.map((target) => [target.runtimeActivity?.url, target.runtimeActivity]));
  const streamEndpoints = new Map(targets.map((target) => [target.activeStreams?.url, target.activeStreams]));
  runtimeEndpoints.delete(undefined);
  streamEndpoints.delete(undefined);
  const deadline = Date.now() + graceMs;
  let failure = null;
  do {
    failure = null;
    for (const endpoint of runtimeEndpoints.values()) {
      const payload = await readJson(endpoint, fetchImpl);
      if (payload?.active?.total !== 0) failure = `runtime activity remained ${payload?.active?.total ?? 'unknown'}`;
    }
    for (const endpoint of streamEndpoints.values()) {
      const payload = await readJson(endpoint, fetchImpl);
      if (!Array.isArray(payload?.streams) || payload.streams.length !== 0) {
        failure = `plugin runner retained ${payload?.streams?.length ?? 'unknown'} active streams`;
      }
    }
    if (!failure || Date.now() >= deadline) break;
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  } while (true);
  if (failure) throw new Error(failure);
  return { runtime_targets: runtimeEndpoints.size, stream_targets: streamEndpoints.size };
}

module.exports = {
  TERMINAL_STATUSES,
  evaluateMockAttempt,
  queryRun,
  readJson,
  reconcileAttempt,
  snapshotRuns,
  verifyIdle,
  waitForBarrierWaiting,
};
