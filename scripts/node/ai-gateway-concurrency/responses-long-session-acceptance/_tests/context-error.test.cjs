'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { isExpectedContextError, validateContextProbe } = require('../context-error.cjs');

function fixture() {
  const digest = { bytes: 100, sha256: 'a'.repeat(64) };
  const meta = { threadId: 'root', contextProbeRequired: true, contextProbeTurnId: 'probe',
    contextProbeAfterTurnId: 'last', contextProbeInput: digest };
  const events = [
    { atMs: 100, method: 'turn/completed', params: { threadId: 'root', turn: { id: 'last', status: 'completed' } } },
    { atMs: 110, kind: 'rpc/request', method: 'turn/start', params: { threadId: 'root' }, contextProbeInput: digest },
    { atMs: 120, kind: 'turn/invoked', threadId: 'root', turnId: 'probe' },
    { atMs: 130, method: 'thread/tokenUsage/updated', params: { threadId: 'root', turnId: 'probe',
      tokenUsage: { modelContextWindow: 1000, total: { totalTokens: 1000 } } } },
    { atMs: 140, method: 'error', params: { threadId: 'root', turnId: 'probe', willRetry: false,
      error: { codexErrorInfo: 'contextWindowExceeded', message: 'The context window is full.' } } },
    { atMs: 150, method: 'turn/completed', params: { threadId: 'root', turn: { id: 'probe', status: 'failed',
      error: { codexErrorInfo: 'contextWindowExceeded', message: 'The context window is full.' } } } },
  ];
  return { meta, events };
}

test('accepts only correlated non-retryable context rejection and full token state', () => {
  const { events, meta } = fixture();
  assert.deepEqual(validateContextProbe(events, meta), { errors: [], verified: true });
  assert.equal(isExpectedContextError(events[4], meta), true);
});

for (const [label, mutate] of [
  ['unknown error code', f => { f.events[4].params.error.codexErrorInfo = 'other'; }],
  ['retries the context refusal', f => { f.events[4].params.willRetry = true; }],
  ['wrong thread', f => { f.events[4].params.threadId = 'other'; }],
  ['wrong turn', f => { f.events[4].params.turnId = 'other'; }],
  ['missing notification', f => { f.events.splice(4, 1); }],
  ['duplicate refusal', f => { f.events.push(structuredClone(f.events[4])); }],
  ['wrong terminal code', f => { f.events[5].params.turn.error.codexErrorInfo = 'other'; }],
  ['successful terminal', f => { f.events[5].params.turn.status = 'completed'; }],
  ['terminal before error', f => { f.events[5].atMs = 135; }],
  ['not full', f => { f.events[3].params.tokenUsage.total.totalTokens = 999; }],
  ['unknown context capacity', f => { f.events[3].params.tokenUsage.modelContextWindow = null; }],
  ['changed input digest', f => { f.meta.contextProbeInput = { bytes: 100, sha256: 'b'.repeat(64) }; }],
  ['probe before final successful work', f => { f.events[0].atMs = 200; }],
  ['unexpected tool', f => { f.events.push({ atMs: 141, method: 'item/completed', params: {
    threadId: 'root', turnId: 'probe', item: { type: 'commandExecution' } } }); }],
]) test(`rejects ${label}`, () => {
  const f = fixture(); mutate(f);
  assert.equal(validateContextProbe(f.events, f.meta).verified, false);
});

test('the exemption requires the explicit negative scenario', () => {
  const { events, meta } = fixture();
  meta.contextProbeRequired = false;
  assert.equal(isExpectedContextError(events[4], meta), false);
});
