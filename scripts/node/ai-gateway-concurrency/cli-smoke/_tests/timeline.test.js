'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const { mergeTimelines } = require('../timeline');

// D3-AC-002/003: every real-client loop consumes the same finite producer/client chronology.
test('Codex, Claude, and OpenCode fixtures prove the barrier chronology from monotonic events', () => {
  const fixtures = path.join(__dirname, 'fixtures');
  const expected = [
    'tool_call', 'client_result', 'second_upstream_request', 'marker_1',
    'barrier_release', 'marker_2', 'terminal',
  ];
  for (const client of ['codex', 'claude', 'opencode']) {
    const events = mergeTimelines(
      path.join(fixtures, `${client}-client.jsonl`),
      path.join(fixtures, `${client}-producer.jsonl`)
    );
    assert.deepEqual(events.map((event) => event.event), expected, client);
    assert.deepEqual(events.map((event) => event.timeline_sequence), [1, 2, 3, 4, 5, 6, 7]);
    assert.equal(events.find((event) => event.event === 'tool_call').source, 'mock-upstream-producer');
    assert.equal(events.find((event) => event.event === 'client_result').source, 'client-pty');
  }
});
