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
  const errorMessage = typeof run?.error?.message === 'string' ? run.error.message : null;
  return {
    id: run?.id ?? null,
    status: run?.status ?? null,
    ...(errorMessage === null ? {} : { error_message: errorMessage }),
  };
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
  expectedStatuses = null,
  expectedErrorBody = null,
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
  if (expectedStatuses) {
    const actual = queried.map((run) => run.status).sort();
    const expected = expectedStatuses.length === expectedRuns
      ? [...expectedStatuses].sort()
      : Array.from({ length: expectedRuns }, () => expectedStatuses[0]).sort();
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(`durable statuses ${actual.join(',')} did not match ${expected.join(',')}`);
    }
  }
  if (expectedErrorBody !== null) {
    for (const run of queried) {
      if (run.error_message !== expectedErrorBody) {
        throw new Error(`durable error for run ${run.id} did not preserve the exact upstream body`);
      }
    }
  }
  return { expected_runs: expectedRuns, runs: queried };
}

function gatewayExecutorEvidence(snapshot, expected = 0) {
  const observed = snapshot?.counters?.gatewayExecutorInvocations;
  if (!Number.isInteger(observed)) throw new Error('mock snapshot omitted gateway executor counter');
  if (observed !== expected) {
    throw new Error(`expected gateway executor=${expected}, observed ${observed}`);
  }
  return { gateway_executor_invocations: observed };
}

function networkObserverEvidence(snapshot, expected = 0) {
  const observed = snapshot?.counters?.networkObserverOutbound;
  if (!Number.isInteger(observed)) throw new Error('mock snapshot omitted network observer counter');
  if (observed !== expected) {
    throw new Error(`expected network observer outbound=${expected}, observed ${observed}`);
  }
  return { network_observer_outbound: observed };
}

function callbackResumeEvidence(events, minimumResumes, callbackResumes, toolMode) {
  const hasMinimum = Number.isInteger(minimumResumes);
  const hasExact = Number.isInteger(callbackResumes);
  if (!hasMinimum && !hasExact) return null;
  if (hasMinimum && hasExact) {
    throw new Error('mock callback resume expectation requires exactly one cardinality rule');
  }
  const calls = events.filter((event) => event.event === 'tool_call');
  const resumes = events.filter((event) => event.event === 'second_upstream_request');
  if (hasExact && resumes.length !== callbackResumes) {
    throw new Error(`expected exactly ${callbackResumes} Gateway callback resume, observed ${resumes.length}`);
  }
  if (hasMinimum && resumes.length < minimumResumes) {
    throw new Error(`expected at least ${minimumResumes} Gateway callback resume, observed ${resumes.length}`);
  }
  const rounds = resumes.map((resume, index) => {
    const call = calls.findLast((candidate) => candidate.sequence < resume.sequence);
    const arrival = events.find((event) => (
      event.event === 'arrival' && event.nonce === resume.nonce && event.sequence < resume.sequence
    ));
    const waiting = events.find((event) => (
      event.event === 'barrier_waiting' && event.nonce === resume.nonce && event.sequence > resume.sequence
    ));
    const released = events.find((event) => (
      event.event === 'barrier_released' && event.nonce === resume.nonce
        && event.sequence > (waiting?.sequence ?? Number.MAX_SAFE_INTEGER)
    ));
    const finalRound = index === resumes.length - 1;
    const barrierRequired = toolMode !== 'sequential_callback_tasks_one_turn' || finalRound;
    const settledAfter = barrierRequired ? released?.sequence : resume.sequence;
    const settled = events.find((event) => (
      event.event === 'settled' && event.nonce === resume.nonce
        && event.sequence > (settledAfter ?? Number.MAX_SAFE_INTEGER)
    ));
    if (!call || !arrival || call.sequence >= arrival.sequence || !settled
      || (barrierRequired && (!waiting || !released))) {
      throw new Error(`Gateway callback resume chronology was incomplete for ${resume.nonce}`);
    }
    if (!barrierRequired && (waiting || released)) {
      throw new Error(`intermediate sequential callback fabricated a text barrier for ${resume.nonce}`);
    }
    return {
      tool_call_sequence: call.sequence,
      callback_request_sequence: resume.sequence,
      barrier_waiting_sequence: waiting?.sequence ?? null,
      barrier_released_sequence: released?.sequence ?? null,
      settled_sequence: settled.sequence,
    };
  });
  const requiredResumes = callbackResumes ?? minimumResumes;
  if (new Set(rounds.map((round) => round.tool_call_sequence)).size < requiredResumes) {
    throw new Error('Gateway callback resumes did not follow distinct Provider tool-call rounds');
  }
  return {
    ...(hasExact ? { exact_resumes: callbackResumes } : { minimum_resumes: minimumResumes }),
    observed_resumes: resumes.length,
    rounds,
  };
}

function evaluateMockAttempt(before, after, rawExpectation) {
  const legacy = typeof rawExpectation === 'number';
  const expectation = legacy
    ? { provider_requests: rawExpectation, provider_outcomes: ['completed'] }
    : rawExpectation;
  const cursor = before?.entries?.at(-1)?.sequence ?? 0;
  const events = (after?.entries ?? []).filter((event) => event.sequence > cursor);
  const arrivals = events.filter((event) => event.event === 'arrival');
  const settled = events.filter((event) => event.event === 'settled');
  const exactRequests = Number.isInteger(expectation.provider_requests);
  const minimumRequests = Number.isInteger(expectation.minimum_provider_requests);
  if (exactRequests === minimumRequests) {
    throw new Error('mock expectation requires exactly one provider request cardinality rule');
  }
  if (exactRequests && arrivals.length !== expectation.provider_requests) {
    throw new Error(`expected ${expectation.provider_requests} mock arrival, observed ${arrivals.length}`);
  }
  if (minimumRequests && arrivals.length < expectation.minimum_provider_requests) {
    throw new Error(
      `expected at least ${expectation.minimum_provider_requests} mock arrival, observed ${arrivals.length}`,
    );
  }
  if (settled.length !== arrivals.length) {
    throw new Error(`expected ${arrivals.length} settled mock request, observed ${settled.length}`);
  }
  const arrivalNonces = new Set(arrivals.map((event) => event.nonce));
  const settledByNonce = new Map(settled.map((event) => [event.nonce, event]));
  if (arrivalNonces.size !== arrivals.length || settledByNonce.size !== settled.length
    || [...settledByNonce.keys()].some((nonce) => !arrivalNonces.has(nonce))) {
    throw new Error('mock arrivals and settled requests were not uniquely paired by nonce');
  }
  for (const [index, arrival] of arrivals.entries()) {
    const event = settledByNonce.get(arrival.nonce);
    if (!event || event.sequence <= arrival.sequence) {
      throw new Error(`mock request ${index + 1} did not settle after its arrival`);
    }
    const expectedOutcome = expectation.provider_outcomes?.[index]
      ?? expectation.provider_outcomes?.[0]
      ?? 'completed';
    if (event.outcome !== expectedOutcome) {
      throw new Error(`mock request ${index + 1} outcome ${event.outcome} did not match ${expectedOutcome}`);
    }
    const expectedTerminalCount = expectation.success_terminal_counts?.[index]
      ?? expectation.success_terminal_counts?.[0];
    if (expectedTerminalCount !== undefined && event.successTerminalCount !== expectedTerminalCount) {
      throw new Error(
        `mock request ${index + 1} success terminals ${event.successTerminalCount}`
          + ` did not match ${expectedTerminalCount}`,
      );
    }
  }
  if (legacy && expectation.provider_requests === 2) {
    const tool = events.findIndex((event) => event.event === 'tool_call');
    const second = events.findIndex((event) => event.event === 'second_upstream_request');
    if (tool === -1 || second <= tool) throw new Error('mock tool two-turn chronology was not observed');
  }
  for (const arrival of arrivals) {
    const requestKeys = arrival.request?.body?.keys ?? [];
    for (const key of expectation.request_body_keys ?? []) {
      if (!requestKeys.includes(key)) throw new Error(`Provider request omitted ${key}`);
    }
    if (expectation.request_body_model !== undefined
      && arrival.request?.body?.model !== expectation.request_body_model) {
      throw new Error(`Provider request model did not match ${expectation.request_body_model}`);
    }
  }
  const executor = expectation.gateway_executor_invocations === undefined
    ? null
    : gatewayExecutorEvidence(after, expectation.gateway_executor_invocations);
  const network = expectation.network_observer_outbound === undefined
    ? null
    : networkObserverEvidence(after, expectation.network_observer_outbound);
  const callback = callbackResumeEvidence(
    events,
    expectation.minimum_callback_resumes,
    expectation.callback_resumes,
    expectation.tool_mode,
  );
  return {
    arrivals: arrivals.length,
    settled: settled.length,
    nonces: arrivals.map((event) => event.nonce),
    outcomes: arrivals.map((event) => settledByNonce.get(event.nonce).outcome),
    success_terminal_counts: arrivals.map(
      (event) => settledByNonce.get(event.nonce).successTerminalCount ?? null,
    ),
    ...(executor || {}),
    ...(network || {}),
    ...(callback ? { callback_resume: callback } : {}),
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
  gatewayExecutorEvidence,
  networkObserverEvidence,
  queryRun,
  readJson,
  reconcileAttempt,
  snapshotRuns,
  verifyIdle,
  waitForBarrierWaiting,
};
