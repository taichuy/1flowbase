'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { evaluate } = require('../oracle.cjs');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rls-oracle-rollouts-'));
test.after(() => fs.rmSync(dir, { recursive: true, force: true }));

function writeRollout(meta, transform = (records) => records) {
  const records = [
    { type: 'session_meta', payload: { id: 'sub', parent_thread_id: 'root' } },
    { type: 'event_msg', payload: { type: 'task_started', turn_id: 'child-turn', root_turn_id: 't0' } },
    { type: 'turn_context', payload: { turn_id: 'child-turn', root_turn_id: 't0',
      model: 'gpt-6-sol', effort: 'medium' } },
    { type: 'event_msg', payload: { type: 'task_complete', turn_id: 'child-turn' } },
  ];
  fs.writeFileSync(meta.subagentRolloutPath, transform(records)
    .map((record) => JSON.stringify(record)).join('\n'));
}

function fixture() {
  const events = [];
  const add = (atMs, method, turnId, item) => events.push({ atMs, method,
    params: { threadId: 'root', turnId, item } });
  for (let i = 0; i <= 6; i++) {
    const id = `t${i}`;
    const start = i * 600_000;
    events.push({ atMs: start, kind: 'turn/invoked', threadId: 'root', turnId: id });
    add(start + 50, 'item/started', id, { type: 'commandExecution' });
    add(start + 100, 'item/completed', id, { type: 'commandExecution', command: 'rg -n example api/crates',
      exitCode: 0, aggregatedOutput: i === 1 ? 'sentinel' : 'real source line' });
    add(start + 200, 'item/completed', id, { type: 'agentMessage', text: `${'Source evidence and counterexample. '.repeat(5)}${i === 1 ? 'steer-nonce' : ''}` });
    events.push({ atMs: start + (i === 0 ? 500 : 300), method: 'turn/completed',
      params: { threadId: 'root', turn: { id, status: 'completed' } } });
  }
  add(250, 'item/completed', 't0', { type: 'subAgentActivity', kind: 'started',
    agentThreadId: 'sub', agentPath: '/root/audit' });
  add(320, 'item/completed', 't0', { type: 'subAgentActivity', kind: 'completed',
    agentThreadId: 'sub', agentPath: '/root/audit' });
  add(400, 'item/completed', 't0', { type: 'agentMessage',
    text: 'Parent continued source analysis and incorporated the completed child finding.' });
  events.push({ atMs: 600, kind: 'rpc/request', method: 'thread/compact/start', params: { threadId: 'root' } });
  add(650, 'item/completed', 'compact', { type: 'contextCompaction' });
  events.push({ atMs: 700, method: 'turn/completed',
    params: { threadId: 'root', turn: { id: 'compact', status: 'completed' } } });
  add(600_010, 'item/agentMessage/delta', 't1', null);
  events.at(-1).params.delta = 'sentinel memory-nonce inspect source > cite evidence > propose counterexample';
  events.push({ atMs: 600_020, kind: 'rpc/request', method: 'turn/steer',
    params: { threadId: 'root', expectedTurnId: 't1' } });
  events.push({ atMs: 600_030, kind: 'steer/accepted', turnId: 't1' });
  const meta = { threadId: 'root', durationMs: 3_600_400, sentinel: 'sentinel', nonce: 'memory-nonce',
    steerNonce: 'steer-nonce',
    instructionOrder: 'inspect source > cite evidence > propose counterexample',
    continuityTurnId: 't1', steerTurnId: 't1', mailboxTurnId: 't0',
    subagentRolloutPath: path.join(dir, `${require('node:crypto').randomUUID()}.jsonl`) };
  writeRollout(meta);
  return { events, meta };
}

test('strong core evidence remains explicitly unverified without mailbox log', () => {
  const { events, meta } = fixture();
  const result = evaluate(events, meta);
  assert.equal(result.status, 'UNVERIFIED');
  assert.equal(result.pass, false);
  assert.deepEqual(result.errors, []);
});

for (const inheritedError of [false, true]) test(`permits inherited parent history (error=${inheritedError})`, () => {
  const { events, meta } = fixture();
  writeRollout(meta, (records) => {
    records.splice(1, 0,
      { type: 'session_meta', payload: { id: 'root' } },
      { type: 'event_msg', payload: { type: 'task_started', turn_id: 't0', root_turn_id: 't0' } },
      { type: 'turn_context', payload: { turn_id: 't0', root_turn_id: 't0', model: 'gpt-6-luna', effort: 'max' } },
      ...(inheritedError ? [{ type: 'event_msg', payload: { type: 'error', message: 'historical parent error' } }] : []));
    return records;
  });
  assert.deepEqual(evaluate(events, meta).errors, []);
});

test('correlated mailbox and gateway reconnection sequence permits PASS', (t) => {
  const { events, meta } = fixture();
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rls-oracle-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const rawLogPath = path.join(dir, 'mailbox.jsonl');
  fs.writeFileSync(rawLogPath, [
    { kind: 'parent/native-sampling-before', rootThreadId: 'root', parentTurnId: 't0',
      flowRunId: 'flow', rolloutId: 'rollout', parentSampleId: 'sample', sourceEventId: 'c1', atMs: 210 },
    { kind: 'mailbox/child-completed', rootThreadId: 'root', parentTurnId: 't0',
      childThreadId: 'sub', sourceEventId: 'c2', atMs: 330 },
    { kind: 'gateway/websocket-reconnected', rootThreadId: 'root', parentTurnId: 't0',
      previousFlowRunId: 'flow', flowRunId: 'next-flow', rolloutId: 'rollout', sourceEventId: 'r1', atMs: 340,
      originalPayload: { flow_run_id: 'next-flow', payload: { event: 'reconnected' } } },
    { kind: 'parent/native-sampling-after', rootThreadId: 'root', parentTurnId: 't0',
      flowRunId: 'next-flow', rolloutId: 'rollout', parentSampleId: 'next-sample', sourceEventId: 'c3', atMs: 350 },
  ].map((e) => JSON.stringify(e)).join('\n'));
  meta.mailboxTurnId = 't0';
  meta.mailboxEvidence = { rawLogPath };
  assert.equal(evaluate(events, meta).status, 'PASS');
});

for (const [label, mutate, expected] of [
  ['less than 60 minutes', (f) => { f.meta.durationMs = 3_599_999; }, 'duration'],
  ['short useful span', (f) => { f.events = f.events.filter((e) => e.params?.turnId !== 't6');
    f.events = f.events.filter((e) => e.turnId !== 't6'); }, 'span'],
  ['idle gap', (f) => { f.events = f.events.filter((e) => !['t3', 't4'].includes(e.params?.turnId)
    && !['t3', 't4'].includes(e.turnId)); }, 'idle'],
  ['no real tool', (f) => { f.events = f.events.filter((e) => e.params?.item?.type !== 'commandExecution'); }, 'tool'],
  ['compact missing', (f) => { f.events = f.events.filter((e) => e.params?.item?.type !== 'contextCompaction'); }, 'compaction'],
  ['post compact continuation missing', (f) => { f.meta.continuityTurnId = 'unknown'; }, 'continuity'],
  ['post compact continuation reversed', (f) => { f.events.find((e) => e.kind === 'turn/invoked'
    && e.turnId === 't1').atMs = 500; }, 'continuity'],
  ['subagent missing', (f) => { f.events = f.events.filter((e) => e.params?.item?.kind !== 'started'); }, 'subagent'],
  ['subagent completion missing', (f) => { f.events = f.events.filter((e) => e.params?.item?.kind !== 'completed'); }, 'subagent'],
  ['subagent rollout missing', (f) => { delete f.meta.subagentRolloutPath; }, 'subagent'],
  ['wrong child completion', (f) => { f.events.find((e) => e.params?.item?.kind === 'completed')
    .params.item.agentThreadId = 'other'; }, 'subagent'],
  ['wrong child parent', (f) => writeRollout(f.meta, (r) => { r[0].payload.parent_thread_id = 'other'; return r; }), 'subagent'],
  ['wrong root turn', (f) => writeRollout(f.meta, (r) => { r[2].payload.root_turn_id = 'other'; return r; }), 'subagent'],
  ['wrong child model', (f) => writeRollout(f.meta, (r) => { r[2].payload.model = 'gpt-6-luna'; return r; }), 'subagent'],
  ['wrong child effort', (f) => writeRollout(f.meta, (r) => { r[2].payload.effort = 'low'; return r; }), 'subagent'],
  ['child terminal missing', (f) => writeRollout(f.meta, (r) => r.slice(0, 3)), 'subagent'],
  ['only inherited parent turn', (f) => writeRollout(f.meta, (r) => {
    for (const e of r.slice(1)) e.payload.turn_id = 't0'; return r;
  }), 'subagent'],
  ['actual child error', (f) => writeRollout(f.meta, (r) => {
    r.splice(3, 0, { type: 'event_msg', payload: { type: 'error', message: 'child failed' } }); return r;
  }), 'subagent'],
  ['steer reversed', (f) => { f.events.find((e) => e.kind === 'steer/accepted').atMs = 600_019; }, 'steer'],
  ['steer after terminal', (f) => { f.events.find((e) => e.kind === 'steer/accepted').atMs = 600_301; }, 'steer'],
  ['steer request missing', (f) => { f.events = f.events.filter((e) => e.method !== 'turn/steer'); }, 'steer'],
  ['runtime close_exhausted', (f) => { f.events.push({ atMs: 2_000_000, kind: 'client/stderr', text: 'sampling_error=stream disconnected: provider_physical_connection_close_exhausted' }); }, 'runtime'],
  ['runtime error', (f) => { f.events.push({ atMs: 2_000_000, method: 'error', params: { threadId: 'root' } }); }, 'runtime'],
]) test(`rejects ${label}`, () => {
  const f = fixture(); mutate(f);
  const result = evaluate(f.events, f.meta);
  assert.equal(result.status, 'FAIL');
  assert.ok(result.errors.some((error) => error.includes(expected)), result.errors.join('; '));
});

test('gateway acceptance does not grade model recall or prose quality', () => {
  const f = fixture();
  f.events = f.events.filter(e => e.method !== 'item/agentMessage/delta');
  for (const e of f.events) {
    if (e.params?.item?.type === 'agentMessage') e.params.item.text = 'ok';
    if (e.params?.turnId === 't1' && e.params.item?.type === 'commandExecution')
      e.params.item.aggregatedOutput = 'source inspected; remembered values were not emitted';
  }
  assert.deepEqual(evaluate(f.events, f.meta).errors, []);
});

test('wrong remembered values and reformatted instructions are not transport errors', () => {
  const f = fixture();
  f.events.find(e => e.method === 'item/agentMessage/delta').params.delta =
    'different-sentinel different-nonce inspect source -> cite evidence -> propose counterexample';
  assert.deepEqual(evaluate(f.events, f.meta).errors, []);
});

test('permits a retry notification only after the same turn completes successfully', () => {
  const f = fixture();
  f.events.push({ atMs: 150, method: 'error', params: { threadId: 'root', turnId: 't0',
    willRetry: true, error: { message: 'Reconnecting... 1/5' } } });
  const result = evaluate(f.events, f.meta);
  assert.deepEqual(result.errors, []);
  assert.equal(result.recoveredRetryNotifications, 1);
});

test('permits actual child stream retry with original successful child terminal', () => {
  const f = fixture();
  writeRollout(f.meta, (records) => {
    records.splice(3, 0, { type: 'event_msg', payload: { type: 'stream_error', message: 'Reconnecting... 1/5' } });
    return records;
  });
  f.events.push({ atMs: 280, method: 'error', params: { threadId: 'sub', turnId: 'child-turn',
    willRetry: true, error: { message: 'Reconnecting... 1/5' } } });
  const result = evaluate(f.events, f.meta);
  assert.deepEqual(result.errors, []);
  assert.equal(result.recoveredRetryNotifications, 1);
});

for (const [label, mutate] of [
  ['final error despite later completion', (event) => { event.params.willRetry = false; }],
  ['unknown retry disposition', (event) => { delete event.params.willRetry; }],
  ['unmatched retry turn', (event) => { event.params.turnId = 'unknown'; }],
  ['unmatched retry thread', (event) => { event.params.threadId = 'other'; }],
  ['close exhaustion even if client recovers', (event) => { event.params.error.message = 'provider_physical_connection_close_exhausted'; }],
]) test(`rejects ${label}`, () => {
  const f = fixture();
  const event = { atMs: 150, method: 'error', params: { threadId: 'root', turnId: 't0',
    willRetry: true, error: { message: 'Reconnecting... 1/5' } } };
  mutate(event);
  f.events.push(event);
  assert.ok(evaluate(f.events, f.meta).errors.some((error) => error.includes('runtime')));
});

test('source quotations of an error code are not runtime failure evidence', () => {
  const f = fixture();
  f.events.push({ atMs: 450, method: 'item/completed', params: { threadId: 'root', turnId: 't0',
    item: { type: 'agentMessage', text: 'Source defines provider_physical_connection_close_exhausted.' } } });
  f.events.find((e) => e.params?.item?.type === 'commandExecution' && e.method === 'item/completed')
    .params.item.aggregatedOutput = 'code path: provider_physical_connection_close_exhausted';
  assert.deepEqual(evaluate(f.events, f.meta).errors, []);
});
