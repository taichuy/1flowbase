'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { orderedUserInput, validateInstructionOrder } = require('../instruction-order.cjs');

test('Native input preserves both user instructions in order, excluding model quotations', () => {
  const body = { native_request: { wire_body: { input: [
    { role: 'assistant', content: [{ text: 'second first' }] },
    { role: 'user', content: [{ type: 'input_text', text: 'first' }] },
    { role: 'user', content: [{ type: 'input_text', text: 'second' }] },
  ] } } };
  assert.equal(orderedUserInput(body, ['first', 'second']), true);
  body.native_request.wire_body.input.reverse();
  assert.equal(orderedUserInput(body, ['first', 'second']), false);
  body.native_request.wire_body.input = [{ role: 'assistant', content: 'first second' }];
  assert.equal(orderedUserInput(body, ['first', 'second']), false);
});

function fixture() {
  const meta = { orderedSteerRequired: true, threadId: 'root', steerTurnId: 'turn', steerNonces: ['first', 'second'],
    instructionOrderEvidence: { status: 'COLLECTED', rootThreadId: 'root', turnId: 'turn',
      sourceEventId: 'event', flowRunId: 'flow', atMs: 50, nonceOrder: ['first', 'second'] } };
  const events = [
    { atMs: 10, kind: 'rpc/request', method: 'turn/steer', params: { threadId: 'root', expectedTurnId: 'turn', input: [{ text: 'first' }] } },
    { atMs: 20, kind: 'steer/accepted', turnId: 'turn', ordinal: 1 },
    { atMs: 30, kind: 'rpc/request', method: 'turn/steer', params: { threadId: 'root', expectedTurnId: 'turn', input: [{ text: 'second' }] } },
    { atMs: 40, kind: 'steer/accepted', turnId: 'turn', ordinal: 2 },
    { atMs: 60, method: 'turn/completed', params: { threadId: 'root', turn: { id: 'turn', status: 'completed' } } },
  ];
  return { events, meta };
}

test('requires accepted ordering and source-backed Native input evidence', () => {
  const { events, meta } = fixture();
  assert.deepEqual(validateInstructionOrder(events, meta), []);
});

for (const [label, mutate] of [
  ['reversed acceptance', f => { f.events[1].atMs = 35; }],
  ['late acceptance', f => { f.events[3].atMs = 70; }],
  ['missing second request', f => { f.events.splice(2, 1); }],
  ['wrong turn', f => { f.events[2].params.expectedTurnId = 'other'; }],
  ['uncollected input', f => { f.meta.instructionOrderEvidence.status = 'UNVERIFIED'; }],
  ['wrong input order', f => { f.meta.instructionOrderEvidence.nonceOrder.reverse(); }],
  ['input before acceptance', f => { f.meta.instructionOrderEvidence.atMs = 35; }],
  ['input after terminal', f => { f.meta.instructionOrderEvidence.atMs = 65; }],
]) test(`rejects ${label}`, () => {
  const f = fixture(); mutate(f);
  assert.ok(validateInstructionOrder(f.events, f.meta).length);
});
