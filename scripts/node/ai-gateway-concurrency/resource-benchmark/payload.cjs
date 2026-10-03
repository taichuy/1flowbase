'use strict';
const crypto = require('node:crypto');
const SIZES = [8192, 131072, 524288];
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
function requestBody(bytes, model) {
  if (!SIZES.includes(bytes)) throw Error('unsupported history size');
  const input = Array.from({ length: 128 }, (_, i) => ({ role: 'user', content: [{ type: 'input_text', text: `${String(i).padStart(3, '0')}|${'h'.repeat(bytes / 128 - 4)}` }] }));
  return JSON.stringify({ model, stream: true, input });
}
const DELTAS = 256;
const TEXT = 'resource-benchmark-fixed|';
function outputOracle(events, expected = TEXT.repeat(DELTAS)) {
  const deltas = events.filter(e => e.type === 'response.output_text.delta');
  const completed = events.filter(e => e.type === 'response.completed');
  const errors = events.filter(e => ['error', 'response.failed', 'response.cancelled', 'response.incomplete'].includes(e.type));
  const actual = deltas.map(e => e.delta).join('');
  const terminalText = completed[0]?.response?.output?.filter(item => item.type === 'message')
    .flatMap(item => item.content || []).filter(part => part.type === 'output_text').map(part => part.text).join('');
  const result = { delta_count: deltas.length, completed_count: completed.length, error_count: errors.length, output_bytes: Buffer.byteLength(actual), output_sha256: hash(actual), expected_sha256: hash(expected) };
  if (deltas.length !== DELTAS || completed.length !== 1 || errors.length || actual !== expected || terminalText !== expected || completed[0]?.response?.status !== 'completed') throw Error('output oracle rejected stream');
  return result;
}
module.exports = { SIZES, DELTAS, TEXT, hash, requestBody, outputOracle };
