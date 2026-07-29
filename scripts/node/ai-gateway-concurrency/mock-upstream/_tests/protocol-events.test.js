'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { TRANSPORT } = require('../../contracts');
const {
  LOSSLESS_LONG_TEXT,
  LOSSLESS_SENTINEL_SEGMENTS,
  anthropicToolEvents,
  chatToolEvents,
  losslessProtocolEvents,
  responsesToolEvents,
} = require('../protocol-events');

test('AC-001: lossless sentinels retain repeated whitespace, Markdown, CJK, emoji, and empty delta', () => {
  assert.ok(LOSSLESS_SENTINEL_SEGMENTS.includes('  '));
  assert.equal(LOSSLESS_SENTINEL_SEGMENTS.filter((segment) => segment === '\n').length, 2);
  assert.ok(LOSSLESS_SENTINEL_SEGMENTS.some((segment) => segment.includes('```markdown')));
  assert.ok(LOSSLESS_SENTINEL_SEGMENTS.some((segment) => segment.includes('中文')));
  assert.ok(LOSSLESS_SENTINEL_SEGMENTS.some((segment) => segment.includes('🙂')));
  assert.ok(LOSSLESS_SENTINEL_SEGMENTS.includes(''));
  assert.ok(LOSSLESS_LONG_TEXT.length > 4096);
});

test('Root #1477 R7 emits client-owned Git commands for all three protocol surfaces', () => {
  const repo = '/home/taichuy/git/1flowbase';
  const command = "git log -2 --oneline && echo '1flowbase-client-tool-result git-log'";
  const responses = responsesToolEvents('git', [repo], false, null, 'shell_command', 'done', [command]);
  const responseItem = responses.chunks.find((chunk) => chunk.type === 'response.output_item.added').item;
  assert.equal(responseItem.name, 'shell_command');
  assert.deepEqual(JSON.parse(responseItem.arguments), { command, workdir: repo });

  const anthropic = anthropicToolEvents('git', [repo], false, 'done', [command]);
  const anthropicTool = anthropic.chunks.find((chunk) => chunk.event === 'content_block_start')
    .data.content_block;
  assert.equal(anthropicTool.name, 'Bash');
  assert.equal(anthropicTool.input.command, command);

  const chat = chatToolEvents('git', [repo], false, 'done', [command]);
  const chatTool = chat.chunks[0].choices[0].delta.tool_calls[0].function;
  assert.equal(chatTool.name, 'bash');
  assert.deepEqual(JSON.parse(chatTool.arguments), {
    command,
    description: 'Inspect the protected Git repository',
  });
});

test('Root #1477 R15 projects only fields declared by each client tool schema', () => {
  const repo = '/home/taichuy/git/1flowbase';
  const command = 'git log -2 --oneline';
  const execCommand = {
    name: 'exec_command',
    parameters: {
      properties: { cmd: { type: 'string' }, workdir: { type: 'string' } },
    },
  };
  const bash = {
    name: 'Bash',
    parameters: { properties: { command: { type: 'string' } } },
  };

  const responses = responsesToolEvents(
    'schema', [repo], false, null, execCommand, 'done', [command],
  );
  const responseItem = responses.chunks.find(
    (chunk) => chunk.type === 'response.output_item.added',
  ).item;
  assert.equal(responseItem.name, 'exec_command');
  assert.deepEqual(JSON.parse(responseItem.arguments), { cmd: command, workdir: repo });

  const anthropic = anthropicToolEvents('schema', [repo], false, 'done', [command], bash);
  const anthropicTool = anthropic.chunks.find(
    (chunk) => chunk.event === 'content_block_start',
  ).data.content_block;
  assert.equal(anthropicTool.name, 'Bash');
  assert.deepEqual(anthropicTool.input, { command });

  const chat = chatToolEvents('schema', [repo], false, 'done', [command], execCommand);
  const chatTool = chat.chunks[0].choices[0].delta.tool_calls[0].function;
  assert.equal(chatTool.name, 'exec_command');
  assert.deepEqual(JSON.parse(chatTool.arguments), { cmd: command, workdir: repo });
});

test('AC-001/006: every provider transport fixture has all deltas and one success terminal', () => {
  for (const transport of Object.values(TRANSPORT)) {
    const stream = losslessProtocolEvents(transport, 'test');
    assert.equal(stream.chunks.filter((chunk) =>
      chunk.type === 'response.output_text.delta'
      || chunk.data?.type === 'content_block_delta'
      || Object.hasOwn(chunk.choices?.[0]?.delta ?? {}, 'content')
    ).length, LOSSLESS_SENTINEL_SEGMENTS.length);
    assert.ok(stream.terminal);
  }
});
