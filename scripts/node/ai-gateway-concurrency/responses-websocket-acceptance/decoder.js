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
  const terminals = events.filter((event) =>
    event?.type === 'response.completed' || event?.type === 'response.failed'
  );
  const terminalCount = terminals.length;
  if (protocolIds.length !== 1) throw new Error(`expected one Gateway response id, received ${protocolIds.length}`);
  const runId = runIdFromResponseId(protocolIds[0]);
  if (!runId) throw new Error('Gateway response id did not contain a durable run UUID');
  if (terminalCount !== 1) throw new Error(`expected one terminal response event, received ${terminalCount}`);
  const terminalType = terminals[0].type;
  if (terminalType === 'response.completed' && upstreamNonces.length !== 1) {
    throw new Error(`expected one controlled upstream nonce, received ${upstreamNonces.length}`);
  }
  const errorMessage = terminalType === 'response.failed' && typeof terminals[0]?.response?.error?.message === 'string'
    ? terminals[0].response.error.message
    : null;
  if (terminalType === 'response.failed' && errorMessage === null) {
    throw new Error('Gateway response.failed omitted error.message');
  }
  return {
    schema_version: '1flowbase.responses-websocket-trace/v1',
    client_trace_id: clientTraceId,
    response_id: protocolIds[0],
    run_id: runId,
    upstream_nonce: upstreamNonces[0] ?? null,
    event_types: eventTypes,
    text_deltas: textDeltas,
    terminal_count: terminalCount,
    ...(terminalType === 'response.failed' ? {
      terminal_type: terminalType,
      error_message: errorMessage,
    } : {}),
  };
}

module.exports = { decodeGatewayFrames, responseId, runIdFromResponseId };
