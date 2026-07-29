'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const test = require('node:test');

const {
  PROTOCOL_CONTEXT_PROFILE_MATRIX,
  assertProfileProjection,
  expectedUpstreamPath,
} = require('../protocol-context-profile-matrix');

function sha256(value) {
  return crypto.createHash('sha256').update(JSON.stringify(value)).digest('hex');
}

function arrival({ path, model, keys, messageCount, url, header = false }) {
  return {
    request: {
      path,
      body: { model, keys, ...(messageCount === undefined ? {} : { messageCount }) },
      fidelity_fixture: {
        url_sha256: sha256(url),
        header_sha256: header ? { 'x-fixture-profile': 'digest-only' } : {},
      },
    },
  };
}

test('Root #1477 AC-013/016: exact profile inventory is the finite three-ingress by three-Provider matrix', () => {
  assert.equal(PROTOCOL_CONTEXT_PROFILE_MATRIX.length, 9);
  assert.deepEqual(
    [...new Set(PROTOCOL_CONTEXT_PROFILE_MATRIX.map((row) => row.source_protocol))],
    ['anthropic_messages', 'openai_chat', 'openai_responses'],
  );
  assert.deepEqual(
    [...new Set(PROTOCOL_CONTEXT_PROFILE_MATRIX.map((row) => row.provider))],
    ['anthropic', 'openai', 'openai_compatible'],
  );
  assert.deepEqual(
    PROTOCOL_CONTEXT_PROFILE_MATRIX.filter((row) => row.residual_restored).map((row) => row.id),
    [
      'anthropic_messages-to-anthropic',
      'openai_chat-to-openai',
      'openai_responses-to-openai',
      'openai_chat-to-openai_compatible',
    ],
  );
});

test('AC-014/016: matching OpenAI Chat Profile restores residual and keeps Typed Native system', () => {
  const row = PROTOCOL_CONTEXT_PROFILE_MATRIX.find(
    (candidate) => candidate.id === 'openai_chat-to-openai',
  );
  const rawCanary = 'must-not-enter-artifact';
  const path = expectedUpstreamPath(row);
  assert.doesNotThrow(() => assertProfileProjection(
    row,
    arrival({
      path,
      model: 'gateway-fixture-model',
      keys: ['fixture_profile_extension', 'messages', 'model', 'stream'],
      messageCount: 2,
      url: `${path}?fixture_query=${rawCanary}`,
      header: true,
    }),
    rawCanary,
    'gateway-fixture-model',
  ));
});

test('AC-014/016: mismatched Anthropic Profile is omitted while system becomes Responses instructions', () => {
  const row = PROTOCOL_CONTEXT_PROFILE_MATRIX.find(
    (candidate) => candidate.id === 'anthropic_messages-to-openai',
  );
  const rawCanary = 'must-not-reach-provider';
  const path = expectedUpstreamPath(row);
  assert.doesNotThrow(() => assertProfileProjection(
    row,
    arrival({
      path,
      model: 'gateway-fixture-model',
      keys: ['input', 'instructions', 'model', 'stream'],
      url: path,
    }),
    rawCanary,
    'gateway-fixture-model',
  ));
});

test('AC-014 controlled negatives: wrong residual projection or lost Typed system fails closed', () => {
  const restored = PROTOCOL_CONTEXT_PROFILE_MATRIX.find(
    (candidate) => candidate.id === 'openai_responses-to-openai',
  );
  const omitted = PROTOCOL_CONTEXT_PROFILE_MATRIX.find(
    (candidate) => candidate.id === 'openai_responses-to-anthropic',
  );
  assert.throws(() => assertProfileProjection(
    restored,
    arrival({
      path: '/v1/responses',
      model: 'gateway-fixture-model',
      keys: ['input', 'instructions', 'model', 'stream'],
      url: '/v1/responses',
    }),
    'missing-residual',
    'gateway-fixture-model',
  ), /residual body\/header projection/u);
  assert.throws(() => assertProfileProjection(
    omitted,
    arrival({
      path: '/v1/messages',
      model: 'gateway-fixture-model',
      keys: ['messages', 'model', 'stream'],
      url: '/v1/messages',
    }),
    'omitted-residual',
    'gateway-fixture-model',
  ), /dropped Typed Native system/u);
});
