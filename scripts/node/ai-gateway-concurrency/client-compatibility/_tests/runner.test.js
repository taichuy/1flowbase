'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { NEW_PROMPT_SENTINEL, assertClientResult, assertDurableTurns, listRunIds } = require('../runner');

test('D7-AC-003/004/005: result requires live marker chronology, completed tool, and new prompt', () => {
  const result = {
    turns: [
      { stop_reason: 'end_turn', text: '1flowbase gateway sentinel ok', tools: [] },
      { stop_reason: 'end_turn', text: 'marker-1 marker-2 1flowbase gateway tool sentinel ok', tools: [{ status: 'completed' }] },
      { stop_reason: 'end_turn', text: NEW_PROMPT_SENTINEL, tools: [] },
    ],
    timeline: [
      { event: 'tool_call' },
      { event: 'text_delta', update: { text: 'marker-1' } },
      { event: 'barrier_release_start' },
      { event: 'barrier_released' },
      { event: 'text_delta', update: { text: 'marker-2' } },
      { event: 'prompt_terminal' },
    ],
  };
  assert.doesNotThrow(() => assertClientResult('fixture', result, [
    { event: 'tool_call' }, { event: 'second_upstream_request' },
  ]));
  const replay = structuredClone(result);
  replay.turns[2].text = replay.turns[0].text;
  assert.throws(() => assertClientResult('fixture', replay, [
    { event: 'tool_call' }, { event: 'second_upstream_request' },
  ]), /new prompt/u);
});

test('D7-AC-005: durable list identities are extracted without content aliases', () => {
  assert.deepEqual([...listRunIds({ data: { items: [{ id: 'run-1' }, { id: 'run-2' }] } })], ['run-1', 'run-2']);
});

test('D7-AC-005: durable prompt evidence requires distinct ids, answers, and usage', () => {
  const runs = [
    { id: 'text', status: 'succeeded', answer: '1flowbase gateway sentinel ok', usage: { output_tokens: 1 } },
    { id: 'tool', status: 'succeeded', answer: '1flowbase gateway tool sentinel ok', usage: { output_tokens: 2 } },
    { id: 'new', status: 'succeeded', answer: NEW_PROMPT_SENTINEL, usage: { output_tokens: 3 } },
  ];
  assert.deepEqual(assertDurableTurns('fixture', runs).map((run) => run.id), ['text', 'tool', 'new']);
  const replay = structuredClone(runs);
  replay[2].usage = replay[0].usage;
  assert.throws(() => assertDurableTurns('fixture', replay), /reused usage/u);
});
