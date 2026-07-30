'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  EPHEMERAL_NO_LEAK_ORACLE,
  REQUEST_FIDELITY_VECTORS,
  REQUEST_NEGATIVE_VECTORS,
  REQUEST_TRANSLATION_VECTORS,
  assertNoEphemeralRawLeak,
  assertRequestPair,
  normalizedRequestFingerprint,
} = require('../request-fidelity');

test('Root #1477 AC-001/004/005: three ingress request fidelity rows are finite and exact', () => {
  assert.deepEqual(REQUEST_FIDELITY_VECTORS.map((row) => row.ingress), [
    'openai_chat', 'anthropic_messages', 'openai_responses',
  ]);
  for (const row of REQUEST_FIDELITY_VECTORS) {
    assert.equal(row.comparison, 'normalized-direct-vs-gateway-sha256');
    assert.equal(row.expected_upstream_path.startsWith('/v1/'), true);
    assert.doesNotMatch(JSON.stringify(row), /api[_-]?key|authorization|cookie/iu);
  }

  assert.deepEqual(REQUEST_NEGATIVE_VECTORS.map((row) => row.kind), [
    'typed-opaque-conflict', 'reserved-field',
  ]);
  assert.equal(REQUEST_NEGATIVE_VECTORS.every((row) => row.expected === 'fail-before-upstream'), true);
  assert.deepEqual(REQUEST_TRANSLATION_VECTORS.map((row) => row.kind), ['foreign-protocol']);
  assert.equal(REQUEST_TRANSLATION_VECTORS[0].expected, 'generate-without-foreign-wire');
  assert.equal(
    REQUEST_TRANSLATION_VECTORS[0].expected_decision,
    'omitted_protocol_context_profile_mismatch',
  );
});

test('Root #1477 AC-005: normalized request comparison ignores only approved wire differences', () => {
  const direct = {
    method: 'POST', url: '/v1/messages?z=2&a=1',
    headers: {
      authorization: 'Bearer direct-secret', host: 'provider.invalid', connection: 'keep-alive',
      'content-type': 'application/json', 'anthropic-beta': 'context-1m-2025-08-07,fixture-safe-beta',
    },
    body: { model: 'fixture-model', thinking: { type: 'adaptive' } },
  };
  const gateway = {
    ...direct, url: '/v1/messages?a=1&z=2',
    headers: {
      ...direct.headers, authorization: 'Bearer gateway-secret', host: '127.0.0.1',
      traceparent: '00-fixture', 'x-request-id': 'gateway-trace',
    },
  };
  assert.equal(normalizedRequestFingerprint(direct), normalizedRequestFingerprint(gateway));
  assert.doesNotThrow(() => assertRequestPair({ direct, gateway }));

  gateway.headers['anthropic-beta'] = [
    'context-1m-2025-08-07',
    'fixture-safe-beta',
  ];
  assert.equal(normalizedRequestFingerprint(direct), normalizedRequestFingerprint(gateway));
  assert.doesNotThrow(() => assertRequestPair({ direct, gateway }));

  gateway.headers['anthropic-beta'] = ['context-1m-2025-08-07', 'different-beta'];
  assert.throws(() => assertRequestPair({ direct, gateway }), /request fidelity mismatch/u);
  gateway.headers['anthropic-beta'] = [
    'context-1m-2025-08-07',
    'fixture-safe-beta',
  ];

  gateway.body = { ...gateway.body, thinking: { type: 'enabled', budget_tokens: 1024 } };
  assert.throws(() => assertRequestPair({ direct, gateway }), /request fidelity mismatch/u);
});

test('Root #1477 AC-006: ephemeral raw canaries and envelope values cannot enter durable evidence', () => {
  assert.equal(EPHEMERAL_NO_LEAK_ORACLE.allowed_projection.includes('digest'), true);
  assert.doesNotThrow(() => assertNoEphemeralRawLeak({
    protocol_context: { locator: 'ephemeral-slot', digest: 'sha256:fixture' },
  }, ['raw-secret-canary']));
  assert.throws(() => assertNoEphemeralRawLeak({ durable: { message: 'raw-secret-canary' } }, [
    'raw-secret-canary',
  ]), /ephemeral raw protocol context leaked/u);
});
