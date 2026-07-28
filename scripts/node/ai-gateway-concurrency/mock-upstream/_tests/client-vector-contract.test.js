'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const {
  CLAUDE_PROTOCOL_SENTINEL,
  CONTINUITY_FINAL_SENTINEL,
  CONTINUITY_SEED_SENTINEL,
  LONG_REPEATED_UNICODE_TEXT,
  GIT_WORKFLOW_FINAL,
  PARALLEL_FINAL_SENTINEL,
  SEQUENTIAL_FINAL_SENTINEL,
  TEXT_SENTINEL,
  textVectorOutput,
  toolVectorFinalOutput,
} = require('../client-vector-contract');

test('BLO-03/04 selects exact long text and continuity output from request history', () => {
  assert.equal(textVectorOutput({
    input: '1flowbase-client-vector=text-long-repeated-unicode',
  }), LONG_REPEATED_UNICODE_TEXT);
  assert.equal(textVectorOutput({ input: `Reply exactly: ${TEXT_SENTINEL}` }), TEXT_SENTINEL);
  assert.equal(textVectorOutput({
    messages: [{ content: '1flowbase-client-vector=conversation-complete-continuity-seed' }],
  }), CONTINUITY_SEED_SENTINEL);

  const check = '1flowbase-client-vector=conversation-complete-continuity-check';
  assert.equal(textVectorOutput({ input: check }), null);
  assert.equal(textVectorOutput({
    messages: [{ role: 'assistant', content: CONTINUITY_SEED_SENTINEL }, { content: check }],
  }), CONTINUITY_FINAL_SENTINEL);
  assert.equal(textVectorOutput(
    { previous_response_id: 'resp_seed', input: check }, new Set(['resp_seed']),
  ), CONTINUITY_FINAL_SENTINEL);
});

test('BLO-05/06/07 requires real Claude profile fields and selects vector-specific tool finals', () => {
  const marker = '1flowbase-client-vector=claude-1m-adaptive-context-management';
  assert.equal(textVectorOutput({ model: 'claude-opus-4-6', messages: [marker] }), null);
  assert.equal(textVectorOutput({
    model: 'claude-opus-4-6',
    messages: [marker],
    thinking: { type: 'adaptive' },
    output_config: { effort: 'high' },
    context_management: { edits: [] },
  }), CLAUDE_PROTOCOL_SENTINEL);
  assert.equal(toolVectorFinalOutput({ input: 'tools-parallel-one-callback-task' }), PARALLEL_FINAL_SENTINEL);
  assert.equal(
    toolVectorFinalOutput({ input: 'tools-sequential-callback-tasks-one-turn' }),
    SEQUENTIAL_FINAL_SENTINEL,
  );
  assert.equal(
    toolVectorFinalOutput({ input: '1flowbase-client-vector=meaningful-git-workflow' }),
    GIT_WORKFLOW_FINAL,
  );
});
