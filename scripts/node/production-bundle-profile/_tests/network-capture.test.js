const test = require('node:test');
const assert = require('node:assert/strict');
const { summarizeNetwork } = require('../browser/network-capture.js');

test('wire accounting includes Worker traffic, cached zeros and unfinished SSE without declaring readiness', () => {
  const result = summarizeNetwork([
    { targetType: 'page', url: '/app.js', end: 10, encodedWireBytes: 300 },
    { targetType: 'worker', url: '/compiler.wasm', end: 11, encodedWireBytes: 700 },
    { targetType: 'page', url: '/cached.js', end: 12, encodedWireBytes: 0, fromCache: true },
    { targetType: 'page', url: '/events', type: 'EventSource' }
  ]);
  assert.equal(result.encodedWireBytes, 1000);
  assert.equal(result.cachedCount, 1);
  assert.equal(result.workerRequests, 1);
  assert.equal(result.unfinished.length, 1);
  assert.deepEqual(result.failures, []);
});
