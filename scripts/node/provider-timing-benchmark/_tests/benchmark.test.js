'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { dryRunSamples, summarize, validateSample } = require('../benchmark');

test('dry-run reports bounded sample counts and nearest-rank p50/p95', () => {
  const result = summarize(dryRunSamples());
  assert.equal(result.variants.direct.sample_count, 5);
  assert.equal(result.variants.gateway.sample_count, 5);
  assert.equal(result.variants.direct.metrics.total.p50_ms, 110);
  assert.equal(result.variants.direct.metrics.total.p95_ms, 130);
  assert.equal(result.variants.gateway.metrics.connect.sample_count, 1);
  assert.equal(result.variants.gateway.metrics.ingress.p50_ms, null);
});

test('input contract rejects payload-bearing or unknown fields', () => {
  assert.throws(() => validateSample({
    variant: 'gateway',
    sample_index: 0,
    metrics_ms: { total: 10 },
    prompt: 'must not be accepted',
  }), /accepts only/);
  assert.throws(() => validateSample({
    variant: 'gateway',
    sample_index: 0,
    metrics_ms: { response_id: 10 },
  }), /unsupported timing metric/);
});
