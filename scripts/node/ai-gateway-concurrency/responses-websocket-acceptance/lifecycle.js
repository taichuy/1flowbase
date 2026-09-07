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
  return { run_id: runIdFromResponseId(ids[0]), client_trace_id: clientTraceId, event_types: events.map((event) => event.type) };
}

async function runGatewayWebSocketLifecycle({ ready, mockSnapshot }, dependencies = {}) {
  const { collectGatewayFrames } = require('../workflow-contract/gateway-websocket');
  const collect = dependencies.collectGatewayFrames ?? collectGatewayFrames;
  const query = dependencies.queryDurableRun ?? queryDurableRun;
  const target = createGatewayTarget(ready);
  const rows = [];
  try {
    const before = mockSnapshot();
    const outcomes = await Promise.allSettled(Array.from({ length: 3 }, async () => {
      const id = `ws-slow-${crypto.randomUUID()}`;
      const frames = await collect(target, id, { inputText: mockScenarioSentinel(SCENARIO.SLOW) });
      const trace = decodeGatewayFrames(frames, { clientTraceId: id });
      const nonces = [...new Set(trace.text_deltas.join(' ').match(/\bmock-\d{6}\b/gu) ?? [])];
      if (nonces.length !== 1 || nonces[0] !== trace.upstream_nonce) throw new Error('slow WS mixed upstream frames');
      const durable = await converge(target, trace, query);
      if (durable.run.status !== 'succeeded') throw new Error('slow WS did not succeed');
      return { trace, durable };
    }));
    const failures = outcomes.filter((outcome) => outcome.status === 'rejected');
    if (failures.length) throw new AggregateError(failures.map((outcome) => outcome.reason), failures.map((outcome) => outcome.reason.message).join('; '));
    const concurrent = outcomes.map((outcome) => outcome.value);
    assertDistinctRequestNonces(concurrent.map((row) => row.trace.upstream_nonce));
    assertDistinctRequestNonces(concurrent.map((row) => row.trace.run_id));
    const arrivals = mockSnapshot().entries.filter((entry) => entry.event === 'arrival' && entry.sequence > (before.entries.at(-1)?.sequence ?? 0));
    if (arrivals.length !== 3 || concurrent.some((row) => !arrivals.some((entry) => entry.nonce === row.trace.upstream_nonce))) {
      throw new Error('slow concurrent WS nonce/upstream correlation mismatch');
    }
    if (!arrivals.some((entry) => entry.active >= 2)) throw new Error('slow WS requests did not overlap at the controlled upstream');
    rows.push({ id: 'slow-concurrency', verdict: 'PASS', connections: concurrent });
  } catch (error) { rows.push({ id: 'slow-concurrency', verdict: 'FAIL', error: error.message }); }
  for (const probe of [{ id: 'client-disconnect', afterDelta: 'disconnect' }, ...CLOSE_PROBES]) {
    try {
      const before = mockSnapshot().entries.at(-1)?.sequence ?? 0;
      const id = `ws-${probe.id}-${crypto.randomUUID()}`;
      const frames = await collect(target, id, { inputText: mockScenarioSentinel(SCENARIO.SLOW), probe });
      let durable = null;
      let trace = null;
      if (probe.closeCode !== undefined && frames.close_code !== probe.closeCode) throw new Error(`${probe.id}: wrong close code`);
      if (probe.afterDelta) {
        trace = interruptedTrace(frames, id);
        durable = await converge(target, trace, query);
        if (durable.run.status === 'succeeded') throw new Error(`${probe.id}: aborted run succeeded`);
      }
      if (!probe.afterDelta) {
        const events = frames.map((chunks) => JSON.parse(Buffer.concat(chunks).toString('utf8')));
        if (events.some(responseId)) throw new Error(`${probe.id}: rejected request created a run`);
        if (mockSnapshot().entries.some((entry) => entry.event === 'arrival' && entry.sequence > before)) throw new Error(`${probe.id}: rejected request reached upstream`);
      }
      rows.push({ id: probe.id, verdict: 'PASS', close_code: frames.close_code ?? null, trace, durable });
    } catch (error) { rows.push({ id: probe.id, verdict: 'FAIL', error: error.message }); }
  }
  try {
    const id = `ws-upstream-interruption-${crypto.randomUUID()}`;
    const trace = decodeGatewayFrames(await collect(target, id, { inputText: mockScenarioSentinel(SCENARIO.STREAM_INTERRUPTION) }), { clientTraceId: id });
    const durable = await converge(target, trace, query);
    if (trace.terminal_type !== 'response.failed' || durable.run.status !== 'failed') throw new Error('upstream interruption did not fail');
    rows.push({ id: 'upstream-interruption', verdict: 'PASS', trace, durable });
  } catch (error) { rows.push({ id: 'upstream-interruption', verdict: 'FAIL', error: error.message }); }
  return { schema_version: '1flowbase.gateway-websocket-lifecycle/v1', verdict: rows.every((row) => row.verdict === 'PASS') ? 'PASS' : 'FAIL', rows };
}

module.exports = { CLOSE_PROBES, converge, interruptedTrace, runGatewayWebSocketLifecycle };
