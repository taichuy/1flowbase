'use strict';

const { TRANSPORT, assertTransport } = require('../contracts');

const DEFAULT_MAX_EVENT_BYTES = 64 * 1024;

// Fixture-side reference decoder only. Runtime/characterize code must keep using
// its owned transport facilities rather than importing this acceptance oracle.

function invalidUtf8(error) {
  const wrapped = new Error(`invalid UTF-8 in provider stream: ${error.message}`);
  wrapped.cause = error;
  return wrapped;
}

function decodeSseChunks(chunks, options = {}) {
  const maxEventBytes = options.maxEventBytes ?? DEFAULT_MAX_EVENT_BYTES;
  const decoder = new TextDecoder('utf-8', { fatal: true });
  const records = [];
  let buffered = '';
  let eventName = null;
  let dataLines = [];
  let eventBytes = 0;

  const dispatch = () => {
    if (dataLines.length === 0) {
      eventName = null;
      eventBytes = 0;
      return;
    }
    const payload = dataLines.join('\n');
    records.push(payload === '[DONE]'
      ? { event: eventName, done: true, data: null }
      : { event: eventName, done: false, data: JSON.parse(payload) });
    eventName = null;
    dataLines = [];
    eventBytes = 0;
  };

  const processLine = (line, delimiterBytes) => {
    eventBytes += Buffer.byteLength(line, 'utf8') + delimiterBytes;
    if (eventBytes > maxEventBytes) {
      throw new Error(`SSE event exceeds ${maxEventBytes} bytes`);
    }
    if (line === '') {
      dispatch();
      return;
    }
    if (line.startsWith(':')) return;
    const colon = line.indexOf(':');
    const field = colon < 0 ? line : line.slice(0, colon);
    let value = colon < 0 ? '' : line.slice(colon + 1);
    if (value.startsWith(' ')) value = value.slice(1);
    if (field === 'event') eventName = value;
    if (field === 'data') dataLines.push(value);
  };

  const drain = (final) => {
    while (buffered.length > 0) {
      const lf = buffered.indexOf('\n');
      const cr = buffered.indexOf('\r');
      let offset;
      if (lf < 0) offset = cr;
      else if (cr < 0) offset = lf;
      else offset = Math.min(lf, cr);
      if (offset < 0) break;
      if (!final && buffered[offset] === '\r' && offset === buffered.length - 1) break;
      const crlf = buffered[offset] === '\r' && buffered[offset + 1] === '\n';
      const delimiterLength = crlf ? 2 : 1;
      processLine(buffered.slice(0, offset), delimiterLength);
      buffered = buffered.slice(offset + delimiterLength);
    }
    if (final && buffered.length > 0) {
      processLine(buffered, 0);
      buffered = '';
    }
  };

  for (const chunk of chunks) {
    try {
      buffered += decoder.decode(chunk, { stream: true });
    } catch (error) {
      throw invalidUtf8(error);
    }
    drain(false);
  }
  try {
    buffered += decoder.decode();
  } catch (error) {
    throw invalidUtf8(error);
  }
  drain(true);
  dispatch();
  return records;
}

function decodeWebSocketFrames(frames, options = {}) {
  const maxEventBytes = options.maxEventBytes ?? DEFAULT_MAX_EVENT_BYTES;
  return frames.map((frameChunks) => {
    const decoder = new TextDecoder('utf-8', { fatal: true });
    const byteLength = frameChunks.reduce((total, chunk) => total + chunk.byteLength, 0);
    if (byteLength > maxEventBytes) throw new Error(`WebSocket event exceeds ${maxEventBytes} bytes`);
    try {
      let payload = '';
      for (const chunk of frameChunks) payload += decoder.decode(chunk, { stream: true });
      payload += decoder.decode();
      return { event: null, done: false, data: JSON.parse(payload) };
    } catch (error) {
      if (error instanceof SyntaxError) throw error;
      throw invalidUtf8(error);
    }
  });
}

function decodeTransport(transport, wire, options = {}) {
  assertTransport(transport);
  if (transport === TRANSPORT.RESPONSES_WEBSOCKET) {
    return decodeWebSocketFrames(wire.frames, options);
  }
  return decodeSseChunks(wire.chunks, options);
}

module.exports = {
  DEFAULT_MAX_EVENT_BYTES,
  decodeSseChunks,
  decodeTransport,
  decodeWebSocketFrames,
};
