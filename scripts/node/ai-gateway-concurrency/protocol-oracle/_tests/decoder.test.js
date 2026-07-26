'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { TRANSPORT } = require('../../contracts');
const { decodeSseChunks, decodeTransport, decodeWebSocketFrames } = require('../decoder');
const {
  LOSSLESS_LONG_TEXT,
  LOSSLESS_SENTINEL_SEGMENTS,
  PARTITIONS,
  partitionWire,
  providerWire,
} = require('../fixtures');
const { canonicalOracle } = require('../oracle-matrix');

for (const transport of Object.values(TRANSPORT)) {
  test(`AC-001: ${transport} preserves the sentinel under every TCP partition`, () => {
    const expected = LOSSLESS_SENTINEL_SEGMENTS.join('');
    for (const widths of Object.values(PARTITIONS)) {
      const wire = partitionWire(providerWire(transport), widths);
      const canonical = canonicalOracle(transport, decodeTransport(transport, wire));
      assert.equal(canonical.text, expected);
      assert.deepEqual(canonical.segments, LOSSLESS_SENTINEL_SEGMENTS);
      assert.equal(canonical.terminalCount, 1);
    }
  });
}

test('AC-001: LF, CRLF, CR, comments, multiline data, and empty delta are lossless', () => {
  const payload = Buffer.from([
    ': comment\r\n\r\n',
    'event: response.output_text.delta\r',
    'data: {\r',
    'data: "type":"response.output_text.delta","delta":""}\r\r',
    'data: [DONE]\n\n',
  ].join(''));
  assert.deepEqual(decodeSseChunks([payload]), [
    {
      event: 'response.output_text.delta',
      done: false,
      data: { type: 'response.output_text.delta', delta: '' },
    },
    { event: null, done: true, data: null },
  ]);
});

test('AC-001: CJK and emoji survive UTF-8 byte splits and long text is unchanged', () => {
  for (const transport of Object.values(TRANSPORT)) {
    const wire = partitionWire(providerWire(transport, [LOSSLESS_LONG_TEXT]), PARTITIONS.bytewise);
    assert.equal(
      canonicalOracle(transport, decodeTransport(transport, wire)).text,
      LOSSLESS_LONG_TEXT,
    );
  }
});

test('AC-001 controlled negatives: invalid UTF-8 and oversized events fail explicitly', () => {
  assert.throws(() => decodeSseChunks([Uint8Array.from([0xff])]), /invalid UTF-8/u);
  assert.throws(() => decodeWebSocketFrames([[Uint8Array.from([0xff])]]), /invalid UTF-8/u);
  assert.throws(
    () => decodeSseChunks([Buffer.from('data: {"x":"too long"}\n\n')], { maxEventBytes: 8 }),
    /exceeds 8 bytes/u,
  );
  assert.throws(
    () => decodeWebSocketFrames([[Buffer.from('{"x":"too long"}')]], { maxEventBytes: 8 }),
    /exceeds 8 bytes/u,
  );
});
