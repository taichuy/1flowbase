'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { TRANSPORT } = require('../../contracts');
const {
  LOSSLESS_LONG_TEXT,
  LOSSLESS_SENTINEL_SEGMENTS,
  losslessProtocolEvents,
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
