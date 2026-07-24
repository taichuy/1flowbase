'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { validateProducerEvents } = require('../timeline');

// D3-AC-002/003: prepared labels cannot stand in for controlled runtime observations.
test('producer chronology rejects labels without a live monotonic timestamp', () => {
  assert.throws(() => validateProducerEvents([{
    schema_version: '1flowbase.ai-gateway-cli-smoke-timeline/v1', event: 'tool_call',
  }]), /monotonic_ns/u);
  assert.throws(() => validateProducerEvents([{
    schema_version: '1flowbase.ai-gateway-cli-smoke-timeline/v1',
    monotonic_ns: '100', event: 'prebaked_chronology',
  }]), /unsupported producer/u);
});
