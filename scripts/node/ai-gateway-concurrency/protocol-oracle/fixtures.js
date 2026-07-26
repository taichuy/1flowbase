'use strict';

const { TRANSPORT } = require('../contracts');
const {
  LOSSLESS_LONG_TEXT,
  LOSSLESS_SENTINEL_SEGMENTS,
  losslessProtocolEvents,
} = require('../mock-upstream/protocol-events');

const encoder = new TextEncoder();

function splitBytes(bytes, widths) {
  const chunks = [];
  let offset = 0;
  let widthIndex = 0;
  while (offset < bytes.byteLength) {
    const width = widths[widthIndex % widths.length];
    chunks.push(bytes.slice(offset, Math.min(offset + width, bytes.byteLength)));
    offset += width;
    widthIndex += 1;
  }
  return chunks;
}

function sseRecord(event, data, lineEnding, multiline = false) {
  const json = JSON.stringify(data);
  const dataLines = multiline
    ? [`data: ${json.slice(0, 1)}`, `data: ${json.slice(1)}`]
    : [`data: ${json}`];
  return [...(event ? [`event: ${event}`] : []), ...dataLines, '', ''].join(lineEnding);
}

function sseBytes(transport, segments) {
  const stream = losslessProtocolEvents(transport, 'oracle', segments);
  const events = [...stream.chunks, ...(Array.isArray(stream.terminal) ? stream.terminal : [stream.terminal])];
  const endings = ['\n', '\r\n', '\r'];
  const records = [': lossless oracle comment\r\n\r\n'];
  events.forEach((entry, index) => {
    const event = entry.event ?? entry.type ?? null;
    records.push(sseRecord(event, entry.data ?? entry, endings[index % endings.length], index === 1));
  });
  if (stream.doneSentinel) records.push('data: [DONE]\n\n');
  return encoder.encode(records.join(''));
}

function providerWire(transport, segments = LOSSLESS_SENTINEL_SEGMENTS) {
  const stream = losslessProtocolEvents(transport, 'oracle', segments);
  if (transport === TRANSPORT.RESPONSES_WEBSOCKET) {
    const entries = [...stream.chunks, stream.terminal];
    return { frames: entries.map((entry) => [encoder.encode(JSON.stringify(entry))]) };
  }
  return { chunks: [sseBytes(transport, segments)] };
}

function partitionWire(wire, widths) {
  if (wire.frames) {
    return { frames: wire.frames.map((frame) => splitBytes(Buffer.concat(frame), widths)) };
  }
  return { chunks: splitBytes(Buffer.concat(wire.chunks), widths) };
}

const PARTITIONS = Object.freeze({
  whole: Object.freeze([Number.MAX_SAFE_INTEGER]),
  bytewise: Object.freeze([1]),
  uneven: Object.freeze([2, 1, 5, 3, 8, 13]),
});

module.exports = {
  LOSSLESS_LONG_TEXT,
  LOSSLESS_SENTINEL_SEGMENTS,
  PARTITIONS,
  partitionWire,
  providerWire,
  splitBytes,
};
