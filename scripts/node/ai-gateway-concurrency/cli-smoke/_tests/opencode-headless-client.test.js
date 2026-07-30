'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  parseSseStream,
  promptBody,
  sessionCreateBody,
} = require('../opencode-headless-client');

test('AC-003/014: OpenCode raw SSE exposes partial text before terminal state', async () => {
  const encoder = new TextEncoder();
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(encoder.encode('data: {"type":"message.part.updated","properties":{"part":{"type":"text","text":"marker-1"}}}\n'));
      controller.enqueue(encoder.encode('\ndata: {"type":"session.status","properties":{"status":{"type":"idle"}}}\n\n'));
      controller.close();
    },
  });
  const events = [];
  for await (const event of parseSseStream(stream)) events.push(event);
  assert.equal(events[0].properties.part.text, 'marker-1');
  assert.equal(events[1].properties.status.type, 'idle');
});

test('AC-014/019: headless session allows only client-owned Read and submits the fixed model', () => {
  const session = sessionCreateBody();
  assert.deepEqual(session.permission[0], { permission: 'read', pattern: '*', action: 'allow' });
  assert.equal(session.permission.some((rule) => rule.permission === 'bash' && rule.action === 'allow'), false);
  assert.deepEqual(promptBody('oneflowbase_gateway/fixture-model', 'sentinel'), {
    agent: 'build',
    model: { providerID: 'oneflowbase_gateway', modelID: 'fixture-model' },
    parts: [{ type: 'text', text: 'sentinel' }],
  });
});

test('Root #1477 R7 grants Bash/Edit only to the isolated meaningful Git vector', () => {
  const session = sessionCreateBody('1flowbase-client-vector=meaningful-git-workflow');
  assert.equal(session.permission.some((rule) => (
    rule.permission === 'bash' && rule.action === 'allow'
  )), true);
  assert.equal(session.permission.some((rule) => (
    rule.permission === 'edit' && rule.action === 'allow'
  )), true);
});
