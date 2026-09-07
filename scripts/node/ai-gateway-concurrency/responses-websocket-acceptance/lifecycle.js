'use strict';

const crypto = require('node:crypto');
const { SCENARIO, mockScenarioSentinel, assertDistinctRequestNonces } = require('../contracts');
const { decodeGatewayFrames, responseId, runIdFromResponseId } = require('./decoder');
const { queryDurableRun } = require('./durable');
const { createGatewayTarget } = require('./target');

const CLOSE_PROBES = Object.freeze([
  { id: 'invalid-json', payload: '{', closeCode: 1007 },
  { id: 'binary', payload: 'binary', opcode: 2, closeCode: 1003 },
  { id: 'unknown-operation', payload: '{"type":"unknown"}', closeCode: 1008 },
  { id: 'unsupported-cancel', payload: '{"type":"response.cancel"}', closeCode: 1008 },
  { id: 'active-second-create', afterDelta: 'second-create', closeCode: 1008 },
]);

async function converge(target, trace, query = queryDurableRun) {
  const deadline = Date.now() + 10_000;
  while (true) {
    try { return await query(target, trace); }
    catch (error) {
      if (!/durable status remained/u.test(error.message) || Date.now() >= deadline) throw error;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
}

function interruptedTrace(frames, clientTraceId) {
  const events = frames.map((chunks) => JSON.parse(Buffer.concat(chunks).toString('utf8')));
  const ids = [...new Set(events.map(responseId).filter(Boolean))];
  if (ids.length !== 1 || !runIdFromResponseId(ids[0])) throw new Error('interrupted WS omitted unique run UUID');
  if (!events.some((event) => event.type === 'response.output_text.delta')) throw new Error('WS did not reach first delta');
  if (events.some((event) => event.type === 'response.completed')) throw new Error('interrupted WS emitted success');
  const nonces = [...new Set(events.filter((event) => event.type === 'response.output_text.delta').map((event) => event.delta).join(' ').match(/\bmock-\d{6}\b/gu) ?? [])];
  if (nonces.length !== 1) throw new Error('interrupted WS omitted unique upstream nonce');
  return { run_id: runIdFromResponseId(ids[0]), upstream_nonce: nonces[0], client_trace_id: clientTraceId, event_types: events.map((event) => event.type) };
}

function timelineSince(snapshot, sequence) {
  return snapshot().entries.filter((entry) => entry.sequence > sequence).map(({ request, ...entry }) => entry);
}

async function readJson(endpoint) {
  const response = await fetch(endpoint.url, {
    method: endpoint.method ?? 'GET', headers: endpoint.headers ?? {}, signal: AbortSignal.timeout(5_000),
  });
  if (!response.ok) throw new Error(`lifecycle evidence HTTP ${response.status}`);
  const body = await response.json();
  return body?.data ?? body;
}

async function connectionSnapshot(provider, expected, read = readJson) {
  const deadline = Date.now() + 10_000;
  while (true) {
    const snapshot = await read(provider.runtime_activity);
    if (snapshot.active?.websocket_connections === expected) return snapshot;
    if (Date.now() >= deadline) throw new Error(`server websocket count did not reach ${expected}`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

async function nativeSnapshot(provider, trace, read = readJson) {
  const template = provider.durable.query_run;
  const native = await read({ ...template, url: template.url_template.replace('{run_id}', trace.run_id) });
  return { id: native.id, status: native.status, error: native.error ?? null, external_trace_id: native.metadata?.external_trace_id ?? null };
}

async function terminalEvidence(provider, trace, row, read = readJson) {
  const deadline = Date.now() + 10_000;
  row.native_polls = [];
  while (true) {
    row.native = await nativeSnapshot(provider, trace, read);
    row.native_polls.push({ at_ns: process.hrtime.bigint().toString(), ...row.native });
    if (['succeeded', 'incomplete', 'failed', 'cancelled'].includes(row.native.status)) break;
    if (Date.now() >= deadline) throw new Error('lifecycle Native run did not converge');
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  const url = new URL(provider.durable.list_runs.url);
  url.search = '';
  url.pathname += `/${trace.run_id}/overview`;
  const overview = await read({ url: url.href, headers: provider.durable.list_runs.headers });
  row.durable = { id: overview.flow_run?.id, status: overview.flow_run?.status, error_payload: overview.flow_run?.error_payload ?? null };
  if (row.native.id !== trace.run_id || row.durable.id !== trace.run_id || row.native.external_trace_id !== trace.client_trace_id) throw new Error('lifecycle run identity/correlation mismatch');
  if (row.native.status !== row.durable.status) throw new Error('lifecycle Native/durable terminal mismatch');
}

async function recoverEvidence(provider, row, read) {
  const ids = [...new Set((row.wire?.events ?? []).map(responseId).filter(Boolean))];
  if (!row.trace && ids.length === 1 && runIdFromResponseId(ids[0])) {
    row.trace = { run_id: runIdFromResponseId(ids[0]), client_trace_id: row.wire.client_trace_id, event_types: row.wire.events.map((event) => event.type) };
  }
  if (!row.trace) return;
  try {
    if (!row.native) row.native = await nativeSnapshot(provider, row.trace, read);
    if (!row.durable) {
      const url = new URL(provider.durable.list_runs.url);
      url.search = '';
      url.pathname += `/${row.trace.run_id}/overview`;
      const overview = await read({ url: url.href, headers: provider.durable.list_runs.headers });
      row.durable = { id: overview.flow_run?.id, status: overview.flow_run?.status, error_payload: overview.flow_run?.error_payload ?? null };
    }
  } catch (error) { row.evidence_error = error.message; }
}

function assertBarrierOrdering(row, expectedStatus) {
  const waiting = row.upstream.filter((entry) => entry.event === 'terminal_barrier_waiting' && entry.barrier_key === row.barrier_key);
  const released = row.upstream.filter((entry) => entry.event === 'terminal_barrier_released' && entry.barrier_key === row.barrier_key);
  if (waiting.length !== 1 || released.length !== 1 || waiting[0].nonce !== row.barrier.nonce) throw new Error('missing unique correlated terminal barrier');
  if (row.trace.upstream_nonce !== row.barrier.nonce) throw new Error('wire/barrier upstream nonce mismatch');
  if (!row.wire.action_ns || BigInt(waiting[0].monotonic_ns) >= BigInt(row.wire.action_ns)) throw new Error('socket/cancel action did not follow the held barrier');
  if (!row.wire.closed_ns || BigInt(row.wire.closed_ns) >= BigInt(released[0].monotonic_ns)) throw new Error('barrier released before socket terminal/close evidence');
  if (row.runtime_after_close?.active?.websocket_connections !== 0) throw new Error('server connection remained open at barrier release');
  if (row.upstream_before_release.some((entry) => entry.event === 'settled' || entry.event === 'terminal_barrier_released')) throw new Error('upstream terminal escaped barrier before server socket close');
  if (row.pre_action_native?.status !== 'running') throw new Error(`business was not running at barrier: ${row.pre_action_native?.status}`);
  const arrivals = row.upstream.filter((entry) => entry.event === 'arrival');
  if (arrivals.length !== 1 || arrivals[0].nonce !== row.barrier.nonce) throw new Error('held WS request did not map to one upstream arrival');
  const settled = row.upstream.filter((entry) => entry.event === 'settled');
  if (settled.length !== 1 || settled[0].sequence <= released[0].sequence) throw new Error('upstream settled before held barrier release');
  if (row.native.status !== expectedStatus || row.durable.status !== expectedStatus) throw new Error(`expected business ${expectedStatus}, observed ${row.native.status}/${row.durable.status}`);
  const types = row.wire.events.map((event) => event.type);
  if (types.includes('response.completed')) throw new Error('held socket action received response.completed');
  if (expectedStatus === 'cancelled' && row.cancel_response?.id !== row.trace.run_id) throw new Error('Native cancellation response run id mismatch');
  if (expectedStatus === 'cancelled' && types.filter((type) => ['response.failed', 'response.cancelled'].includes(type)).length !== 1) throw new Error('Native cancellation omitted unique failed/cancelled projection');
  if (expectedStatus === 'succeeded' && settled[0].successTerminalCount !== 1) throw new Error('delivery disconnect did not preserve one successful upstream terminal');
}

async function runGatewayWebSocketLifecycle({ ready, mockSnapshot, terminalBarriers }, dependencies = {}) {
  const { collectGatewayFrames } = require('../workflow-contract/gateway-websocket');
  const collect = dependencies.collectGatewayFrames ?? collectGatewayFrames;
  const read = dependencies.readJson ?? readJson;
  const provider = ready.targets.openai;
  const target = createGatewayTarget(ready);
  const rows = [];
  async function execute(id, work) {
    const row = { id, verdict: 'FAIL', wire: { events: [] } };
    const before = mockSnapshot().entries.at(-1)?.sequence ?? 0;
    rows.push(row);
    try { await work(row, before); row.verdict = 'PASS'; }
    catch (error) { row.error = error.message; await recoverEvidence(provider, row, read); }
    finally { row.upstream = timelineSince(mockSnapshot, before); }
  }

  await execute('slow-concurrency', async (row, before) => {
    row.topology = { application_id: target.application_id, provider_instance_id: target.provider_instance_id, upstream_policy: 'same-pool-serialized' };
    row.connections = Array.from({ length: 3 }, () => ({ wire: {} }));
    let opened = 0;
    let release;
    let reject;
    const gate = new Promise((resolve, fail) => { release = resolve; reject = fail; });
    gate.catch(() => {}); // A failed upgrade may precede every rendezvous subscriber.
    const onOpen = async () => {
      opened += 1;
      if (opened === 3) {
        try {
          row.runtime_activity = await connectionSnapshot(provider, 3, read);
          release();
        } catch (error) { reject(error); }
      }
      await gate;
    };
    const outcomes = await Promise.allSettled(row.connections.map(async (connection) => {
      const id = `ws-slow-${crypto.randomUUID()}`;
      try {
        const frames = await collect(target, id, { inputText: mockScenarioSentinel(SCENARIO.SLOW), observation: connection.wire, onOpen });
        connection.trace = decodeGatewayFrames(frames, { clientTraceId: id });
        await terminalEvidence(provider, connection.trace, connection, read);
        const nonces = [...new Set(connection.trace.text_deltas.join(' ').match(/\bmock-\d{6}\b/gu) ?? [])];
        if (nonces.length !== 1 || nonces[0] !== connection.trace.upstream_nonce) throw new Error('slow WS mixed upstream frames');
        if (connection.native.status !== 'succeeded') throw new Error('slow WS did not succeed');
      } catch (error) { connection.error = error.message; reject(error); await recoverEvidence(provider, connection, read); throw error; }
    }));
    const failures = outcomes.filter((outcome) => outcome.status === 'rejected');
    if (failures.length) throw new Error(failures.map((outcome) => outcome.reason.message).join('; '));
    row.upstream = timelineSince(mockSnapshot, before);
    const arrivals = row.upstream.filter((entry) => entry.event === 'arrival');
    if (arrivals.length !== 3 || row.connections.some((connection) => !arrivals.some((entry) => entry.nonce === connection.trace.upstream_nonce))) throw new Error('concurrent Gateway run/upstream nonce mismatch');
    assertDistinctRequestNonces(row.connections.map((connection) => connection.trace.upstream_nonce));
    assertDistinctRequestNonces(row.connections.map((connection) => connection.trace.run_id));
    const lastOpen = row.connections.map((connection) => BigInt(connection.wire.opened_ns)).reduce((left, right) => left > right ? left : right);
    if (row.connections.some((connection) => BigInt(connection.wire.closed_ns) <= lastOpen)) throw new Error('Gateway socket lifetimes did not overlap');
  });

  for (const probe of [{ id: 'client-disconnect', afterDelta: 'disconnect' }, ...CLOSE_PROBES, { id: 'native-cancel' }]) {
    await execute(probe.id, async (row, before) => {
      const held = Boolean(probe.afterDelta) || probe.id === 'native-cancel';
      const id = `ws-${probe.id}-${crypto.randomUUID()}`;
      let input = mockScenarioSentinel(SCENARIO.SLOW);
      if (held) {
        row.barrier_key = crypto.randomUUID();
        input += ` ${terminalBarriers.arm(row.barrier_key)}`;
      }
      try {
        const frames = await collect(target, id, {
          inputText: input, probe, observation: row.wire,
          onFirstDelta: held ? async ({ frames: current }) => {
            row.trace = interruptedTrace(current, id);
            row.barrier = await terminalBarriers.wait(row.barrier_key);
            row.upstream_before_action = timelineSince(mockSnapshot, before);
            row.pre_action_native = await nativeSnapshot(provider, row.trace, read);
            if (row.pre_action_native.status !== 'running') throw new Error('held run was not running before socket/cancel action');
            if (probe.id === 'native-cancel') {
              row.wire.action = 'native-cancel';
              row.wire.action_ns = process.hrtime.bigint().toString();
              const endpoint = provider.durable.cancel_run;
              const response = await read({ ...endpoint, url: endpoint.url_template.replace('{run_id}', row.trace.run_id) });
              row.cancel_response = { id: response.id, status: response.status, error: response.error ?? null };
            }
          } : undefined,
        });
        row.close_code = frames.close_code ?? null;
        if (held) {
          row.trace = interruptedTrace(frames, id);
          row.runtime_after_close = await connectionSnapshot(provider, 0, read);
          row.upstream_before_release = timelineSince(mockSnapshot, before);
          terminalBarriers.release(row.barrier_key);
          await terminalBarriers.waitSettled(row.barrier_key);
          await terminalEvidence(provider, row.trace, row, read);
          row.upstream = timelineSince(mockSnapshot, before);
          assertBarrierOrdering(row, probe.id === 'native-cancel' ? 'cancelled' : 'succeeded');
        } else {
          if (row.wire.events.some(responseId)) throw new Error('rejected request created a run');
          if (timelineSince(mockSnapshot, before).some((entry) => entry.event === 'arrival')) throw new Error('rejected request reached upstream');
        }
        if (probe.closeCode !== undefined && frames.close_code !== probe.closeCode) throw new Error('unexpected protocol close code');
      } finally {
        if (held) {
          terminalBarriers.release(row.barrier_key);
          try { await terminalBarriers.waitSettled(row.barrier_key); } catch (error) { row.cleanup_error = error.message; }
          if (row.trace && !row.native) {
            try { await terminalEvidence(provider, row.trace, row, read); } catch (error) { row.evidence_error = error.message; }
          }
        }
      }
    });
  }

  await execute('upstream-interruption', async (row) => {
    const id = `ws-upstream-interruption-${crypto.randomUUID()}`;
    const frames = await collect(target, id, { inputText: mockScenarioSentinel(SCENARIO.STREAM_INTERRUPTION), observation: row.wire });
    row.trace = decodeGatewayFrames(frames, { clientTraceId: id });
    await terminalEvidence(provider, row.trace, row, read);
    if (row.trace.terminal_type !== 'response.failed' || row.native.status !== 'failed') throw new Error('upstream interruption did not fail');
  });
  return { schema_version: '1flowbase.gateway-websocket-lifecycle/v1', verdict: rows.every((row) => row.verdict === 'PASS') ? 'PASS' : 'FAIL', rows };
}

module.exports = { CLOSE_PROBES, converge, interruptedTrace, assertBarrierOrdering, terminalEvidence, runGatewayWebSocketLifecycle };
