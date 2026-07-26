'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { TRANSPORT } = require('../../contracts');
const {
  LIFECYCLE_PAIRWISE_ORACLES,
  PROTOCOL_TRANSPORT_ORACLES,
  PROVIDER_TRANSPORTS,
  PUBLIC_PROTOCOLS,
  TRANSPORT_DECODER_ORACLES,
} = require('../oracle-matrix');

test('AC-001/006: decoder, canonical, and durable oracles cover the 4 x 4 matrix', () => {
  assert.equal(PUBLIC_PROTOCOLS.length, 4);
  assert.equal(PROVIDER_TRANSPORTS.length, 4);
  assert.equal(PROTOCOL_TRANSPORT_ORACLES.length, 16);
  assert.equal(new Set(PROTOCOL_TRANSPORT_ORACLES.map((row) => row.id)).size, 16);
  assert.ok(PROVIDER_TRANSPORTS.includes(TRANSPORT.CHAT_COMPLETIONS_SSE));
  assert.equal(Object.keys(TRANSPORT_DECODER_ORACLES).length, 4);
  for (const row of PROTOCOL_TRANSPORT_ORACLES) {
    assert.equal(row.decoder.utf8, 'fatal');
    assert.equal(row.canonical.text, row.durable.text);
    assert.equal(row.canonical.terminalCount, 1);
    assert.equal(row.durable.preservesRepeatedContent, true);
  }
});

test('AC-006: every public protocol/provider transport lifecycle pair is absorbing', () => {
  assert.equal(LIFECYCLE_PAIRWISE_ORACLES.length, 16);
  for (const row of LIFECYCLE_PAIRWISE_ORACLES) {
    assert.deepEqual(row.states, ['accepted', 'streaming', 'terminal', 'durable']);
    assert.equal(row.terminalIsAbsorbing, true);
    assert.equal(row.lateEvent, 'reject');
    assert.match(row.disconnectBeforeTerminal, /no-success-terminal/u);
    assert.match(row.cancelBeforeTerminal, /no-success-terminal/u);
  }
});
