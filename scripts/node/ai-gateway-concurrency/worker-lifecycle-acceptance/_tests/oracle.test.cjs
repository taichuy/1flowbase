'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { evaluate, assertKillIdentity } = require('../oracle.cjs');
function fixture() {
  const events = [];
  const add = (method, turnId, item, atMs = 0) => events.push({ method, atMs, params: { threadId: 'root', turnId, item } });
  for (let i = 0; i < 8; i++) {
    const id = `t${i}`; const atMs = i * 800000;
    events.push({ kind: 'turn/invoked', turnId: id });
    add('item/completed', id, { type: 'commandExecution', exitCode: 0 }, atMs);
    add('item/completed', id, { type: 'agentMessage', text: `${'Evidence-based source trace '.repeat(8)}${i === 1 ? 'nonce' : ''}` }, atMs);
    events.push({ method: 'turn/completed', atMs, params: { threadId: 'root', turn: { id, status: 'completed' } } });
  }
  add('item/completed', 't0', { type: 'collabAgentToolCall', tool: 'spawnAgent', status: 'completed', receiverThreadIds: ['child'] });
  add('item/completed', 't0', { type: 'collabAgentToolCall', tool: 'wait', status: 'completed', agentsStates: { child: { status: 'completed' } } });
  add('item/completed', 'compact', { type: 'contextCompaction' });
  events.push({ kind: 'rpc/request', method: 'thread/compact/start' });
  events.push({ kind: 'steer/accepted', turnId: 't1' });
  events.push({ method: 'turn/completed', atMs: 100, params: { threadId: 'root', turn: { id: 'killed', status: 'failed' } } });
  const meta = { threadId: 'root', durationMs: 5600000, nonce: 'nonce', sentinelBefore: 'hash', sentinelAfter: 'hash',
    betweenKill: { before: { pid: 20 }, after: { pid: 21 } },
    midKill: { before: { pid: 21 }, after: { pid: 22 }, turnId: 'killed', semanticSeen: true, atMs: 90 } };
  return { events, meta };
}
test('complete real-protocol-shaped evidence passes oracle', () => {
  const { events, meta } = fixture(); assert.equal(evaluate(events, meta).pass, true);
});
for (const [label, mutate, fragment] of [
  ['subagent', (f) => { f.events = f.events.filter((e) => e.params?.item?.type !== 'collabAgentToolCall'); }, 'subagent'],
  ['compact completion', (f) => { f.events = f.events.filter((e) => e.params?.item?.type !== 'contextCompaction'); }, 'compaction'],
  ['steer processed', (f) => { f.meta.nonce = 'never-processed'; }, 'steer'],
  ['minimum duration', (f) => { f.meta.durationMs = 5399999; }, 'duration'],
  ['useful work span', (f) => { f.events.find((e) => e.method === 'turn/completed' && e.params.turn.id === 't7').atMs = 5300000; }, 'span'],
  ['real useful tools', (f) => { f.events = f.events.filter((e) => e.params?.item?.type !== 'commandExecution'); }, 'tool-backed'],
  ['continuity', (f) => { f.events.find((e) => e.method === 'turn/completed' && e.params.turn.id === 't7').atMs = 9999999; }, 'gap'],
  ['failure exactly once', (f) => { f.events.push(f.events.find((e) => e.params?.turn?.id === 'killed')); }, 'once'],
  ['no hidden retry', (f) => { f.events.push({ method: 'error', params: { threadId: 'root', turnId: 'killed', willRetry: true } }); }, 'retry'],
  ['new supplier generation', (f) => { f.meta.midKill.after.pid = 21; }, 'replacement'],
  ['semantic output', (f) => { f.meta.midKill.semanticSeen = false; }, 'semantic'],
]) test(`rejects missing/invalid ${label}`, () => {
  const f = fixture(); mutate(f); const result = evaluate(f.events, f.meta);
  assert.equal(result.pass, false); assert.ok(result.errors.some((error) => error.includes(fragment)), result.errors.join('; '));
});
test('kill selection requires exact live supplier ownership and start time', () => {
  const expected = { parentPid: 10, parentExe: '/candidate/api', parentArg: '--candidate-7600',
    workerExe: '/installed/openai/0.2.66/bin/openai-provider', workerArg: '--stdio', workerDigest: 'digest', startTime: '100' };
  const parent = { pid: 10, exe: expected.parentExe, argv: [expected.parentArg] };
  const worker = { pid: 20, ppid: 10, exe: expected.workerExe, argv: ['--stdio'], digest: 'digest', startTime: '100' };
  assert.equal(assertKillIdentity(worker, expected, parent), 20);
  for (const bad of [{ pid: 10 }, { ppid: 11 }, { exe: '/unrelated/plugin' }, { digest: 'other-build' },
    { startTime: '101' }, { argv: ['--other'] }]) assert.throws(() => assertKillIdentity({ ...worker, ...bad }, expected, parent), /Unsafe/);
  assert.throws(() => assertKillIdentity(worker, expected, { ...parent, argv: ['--candidate-7600', '--port=7800'] }), /Unsafe/);
});
