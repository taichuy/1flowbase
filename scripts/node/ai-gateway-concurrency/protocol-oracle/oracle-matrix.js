'use strict';

const { PUBLIC_PROTOCOL, SUCCESS_TERMINAL, TRANSPORT } = require('../contracts');
const { LOSSLESS_SENTINEL_SEGMENTS, PARTITIONS } = require('./fixtures');

const PUBLIC_PROTOCOLS = Object.freeze(Object.values(PUBLIC_PROTOCOL));
const PROVIDER_TRANSPORTS = Object.freeze(Object.values(TRANSPORT));
const TRANSPORT_DECODER_ORACLES = Object.freeze({
  [TRANSPORT.RESPONSES_SSE]: Object.freeze({ framing: 'sse', payload: 'json', utf8: 'fatal' }),
  [TRANSPORT.RESPONSES_WEBSOCKET]: Object.freeze({ framing: 'websocket-message', payload: 'json', utf8: 'fatal' }),
  [TRANSPORT.CHAT_COMPLETIONS_SSE]: Object.freeze({ framing: 'sse', payload: 'json-or-done', utf8: 'fatal' }),
  [TRANSPORT.ANTHROPIC_SSE]: Object.freeze({ framing: 'sse', payload: 'event-and-json', utf8: 'fatal' }),
});

function eventType(record) {
  return record.event ?? record.data?.type ?? (record.data?.choices ? 'chat.completion.chunk' : null);
}

function canonicalOracle(transport, records) {
  const segments = [];
  let terminalCount = 0;
  for (const record of records) {
    const data = record.data;
    if (data?.type === 'response.output_text.delta') segments.push(data.delta);
    if (data?.type === 'content_block_delta' && data.delta?.type === 'text_delta') {
      segments.push(data.delta.text);
    }
    if (data?.choices?.[0]?.delta && Object.hasOwn(data.choices[0].delta, 'content')) {
      segments.push(data.choices[0].delta.content);
    }
    if (eventType(record) === SUCCESS_TERMINAL[transport]) terminalCount += 1;
    if (transport === TRANSPORT.CHAT_COMPLETIONS_SSE && data?.choices?.[0]?.finish_reason) {
      terminalCount += 1;
    }
  }
  return { segments, text: segments.join(''), terminal: 'finished', terminalCount };
}

const expectedText = LOSSLESS_SENTINEL_SEGMENTS.join('');
const PROTOCOL_TRANSPORT_ORACLES = Object.freeze(PUBLIC_PROTOCOLS.flatMap((publicProtocol) =>
  PROVIDER_TRANSPORTS.map((providerTransport) => Object.freeze({
    id: `${publicProtocol}::${providerTransport}`,
    publicProtocol,
    providerTransport,
    decoder: TRANSPORT_DECODER_ORACLES[providerTransport],
    canonical: Object.freeze({
      segments: LOSSLESS_SENTINEL_SEGMENTS,
      text: expectedText,
      terminal: 'finished',
      terminalCount: 1,
    }),
    durable: Object.freeze({
      text: expectedText,
      terminal: 'finished',
      terminalCount: 1,
      preservesRepeatedContent: true,
    }),
  }))));

const LIFECYCLE_PAIRWISE_ORACLES = Object.freeze(PROTOCOL_TRANSPORT_ORACLES.map((oracle) => Object.freeze({
  id: oracle.id,
  ingressProtocol: oracle.publicProtocol,
  providerTransport: oracle.providerTransport,
  states: Object.freeze(['accepted', 'streaming', 'terminal', 'durable']),
  terminalIsAbsorbing: true,
  lateEvent: 'reject',
  disconnectBeforeTerminal: 'failed-no-success-terminal',
  cancelBeforeTerminal: 'cancelled-no-success-terminal',
})));

const CANONICAL_STREAM_REGRESSION_ORACLE = Object.freeze({
  origin: 'Root #1461',
  providerTransports: PROVIDER_TRANSPORTS,
  partitions: Object.freeze(Object.keys(PARTITIONS)),
  utf8: 'fatal-across-provider-chunk-boundaries',
  delivery: 'write-each-complete-provider-event-immediately',
  ordering: 'exact-segment-order-with-repetitions',
  successTerminalCount: 1,
  terminalIsAbsorbing: true,
  durableParity: Object.freeze({
    text: LOSSLESS_SENTINEL_SEGMENTS.join(''),
    terminal: 'finished',
    preservesRepeatedContent: true,
  }),
});

module.exports = {
  CANONICAL_STREAM_REGRESSION_ORACLE,
  LIFECYCLE_PAIRWISE_ORACLES,
  PROTOCOL_TRANSPORT_ORACLES,
  PROVIDER_TRANSPORTS,
  PUBLIC_PROTOCOLS,
  TRANSPORT_DECODER_ORACLES,
  canonicalOracle,
};
