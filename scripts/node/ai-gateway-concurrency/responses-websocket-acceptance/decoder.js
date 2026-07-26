'use strict';

const { decodeWebSocketFrames } = require('../protocol-oracle/decoder');

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;

function responseId(event) {
  return [event?.response?.id, event?.response_id]
    .find((value) => typeof value === 'string' && value.startsWith('resp_')) ?? null;
}

function runIdFromResponseId(protocolId) {
  if (typeof protocolId !== 'string' || !protocolId.startsWith('resp_')) return null;
  const runId = protocolId.slice('resp_'.length);
  return UUID_PATTERN.test(runId) ? runId : null;
}

function upstreamNonce(text) {
  return typeof text === 'string' ? text.match(/\bmock-\d{6}\b/u)?.[0] ?? null : null;
}

function decodeGatewayFrames(frames, { clientTraceId, maxEventBytes } = {}) {
  if (typeof clientTraceId !== 'string' || clientTraceId.trim() === '') {
    throw new Error('Responses WebSocket client trace id is required');
  }
  const records = decodeWebSocketFrames(frames, { maxEventBytes });
  const events = records.map((record) => record.data);
  const eventTypes = events.map((event) => event?.type ?? null);
  const protocolIds = [...new Set(events.map(responseId).filter(Boolean))];
  const textDeltas = events
    .filter((event) => event?.type === 'response.output_text.delta' && typeof event.delta === 'string')
    .map((event) => event.delta);
  const upstreamNonces = [...new Set(textDeltas.map(upstreamNonce).filter(Boolean))];
  const terminalCount = eventTypes.filter((type) => type === 'response.completed').length;
  if (protocolIds.length !== 1) throw new Error(`expected one Gateway response id, received ${protocolIds.length}`);
  const runId = runIdFromResponseId(protocolIds[0]);
  if (!runId) throw new Error('Gateway response id did not contain a durable run UUID');
  if (terminalCount !== 1) throw new Error(`expected one response.completed, received ${terminalCount}`);
  if (upstreamNonces.length !== 1) throw new Error(`expected one controlled upstream nonce, received ${upstreamNonces.length}`);
  return {
    schema_version: '1flowbase.responses-websocket-trace/v1',
    client_trace_id: clientTraceId,
    response_id: protocolIds[0],
    run_id: runId,
    upstream_nonce: upstreamNonces[0],
    event_types: eventTypes,
    text_deltas: textDeltas,
    terminal_count: terminalCount,
  };
}

module.exports = { decodeGatewayFrames, responseId, runIdFromResponseId };
