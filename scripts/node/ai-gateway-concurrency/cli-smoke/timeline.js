'use strict';

const fs = require('node:fs');

const TIMELINE_SCHEMA = '1flowbase.ai-gateway-cli-smoke-timeline/v1';
const PRODUCER_EVENTS = new Set(['tool_call', 'second_upstream_request']);

function appendTimelineEvent(filePath, event, fields = {}) {
  fs.appendFileSync(filePath, `${JSON.stringify({
    schema_version: TIMELINE_SCHEMA,
    monotonic_ns: process.hrtime.bigint().toString(),
    event,
    ...fields,
  })}\n`, { mode: 0o600 });
}

function readTimeline(filePath) {
  if (!filePath || !fs.existsSync(filePath)) return [];
  return fs.readFileSync(filePath, 'utf8').split(/\r?\n/u).filter(Boolean).map((line) => JSON.parse(line));
}

function validateProducerEvents(events) {
  for (const event of events) {
    if (event.schema_version !== TIMELINE_SCHEMA || !PRODUCER_EVENTS.has(event.event)) {
      throw new Error(`unsupported producer timeline event: ${event.event ?? '<missing>'}`);
    }
    if (!/^[0-9]+$/u.test(event.monotonic_ns)) {
      throw new Error('producer timeline event omitted monotonic_ns');
    }
  }
  return events;
}

function mergeTimelines(clientPath, producerPath) {
  const client = readTimeline(clientPath);
  const producer = validateProducerEvents(readTimeline(producerPath)).map((event) => ({
    ...event,
    source: 'mock-upstream-producer',
  }));
  return [...client, ...producer].sort((left, right) => {
    const time = BigInt(left.monotonic_ns) - BigInt(right.monotonic_ns);
    if (time !== 0n) return time < 0n ? -1 : 1;
    const offset = (left.stream_offset ?? -1) - (right.stream_offset ?? -1);
    if (offset !== 0) return offset;
    return String(left.event).localeCompare(String(right.event));
  }).map((event, index) => ({ ...event, timeline_sequence: index + 1 }));
}

function writeMergedTimeline(clientPath, producerPath, secrets = []) {
  const events = mergeTimelines(clientPath, producerPath);
  const serialized = secrets.reduce(
    (text, secret) => secret ? text.split(secret).join('<redacted-application-key>') : text,
    events.map((event) => JSON.stringify(event)).join('\n') + '\n'
  );
  fs.writeFileSync(clientPath, serialized, { mode: 0o600 });
  return events;
}

module.exports = {
  TIMELINE_SCHEMA,
  appendTimelineEvent,
  mergeTimelines,
  readTimeline,
  validateProducerEvents,
  writeMergedTimeline,
};
