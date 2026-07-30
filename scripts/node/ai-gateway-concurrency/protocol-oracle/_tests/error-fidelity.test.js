'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { UPSTREAM_ERROR_FIXTURES, assertUpstreamErrorFidelity } = require('../error-fidelity');

test('Root #1477 AC-008: JSON/text/HTML/empty/retry error fixtures are finite and byte exact', () => {
  assert.deepEqual(UPSTREAM_ERROR_FIXTURES.map((row) => row.id), [
    'json', 'text', 'html', 'empty', 'retry',
  ]);
  for (const fixture of UPSTREAM_ERROR_FIXTURES.filter((row) => row.body.length > 0)) {
    assert.doesNotThrow(() => assertUpstreamErrorFidelity(fixture, {
      nativeMessage: fixture.body,
      durableMessage: fixture.body,
      clientMessages: [fixture.body, fixture.body, fixture.body, fixture.body],
    }));
  }
});

test('Root #1477 AC-008 controlled negatives reject trimmed, decoded, or selected error bodies', () => {
  const fixture = UPSTREAM_ERROR_FIXTURES.find((row) => row.id === 'json');
  assert.throws(() => assertUpstreamErrorFidelity(fixture, {
    nativeMessage: fixture.body.trim(), durableMessage: fixture.body,
    clientMessages: [fixture.body],
  }), /Native error message/u);

  const empty = UPSTREAM_ERROR_FIXTURES.find((row) => row.id === 'empty');
  assert.doesNotThrow(() => assertUpstreamErrorFidelity(empty, {
    nativeMessage: 'upstream returned HTTP 503',
    durableMessage: 'upstream returned HTTP 503',
    clientMessages: Array(4).fill('upstream returned HTTP 503'),
  }));
  assert.throws(() => assertUpstreamErrorFidelity(empty, {
    nativeMessage: 'HTTP 503', durableMessage: 'provider failed', clientMessages: ['HTTP 503'],
  }), /empty-body fallback/u);
});
