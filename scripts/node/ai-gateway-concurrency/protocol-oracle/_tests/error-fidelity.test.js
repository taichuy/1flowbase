'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { UPSTREAM_ERROR_FIXTURES, ERROR_FIDELITY_ROWS, ERROR_SURFACES, assertErrorFidelityRowInventory, assertUpstreamErrorFidelity } = require('../error-fidelity');

test('Root #1477 AC-008: JSON/text/HTML/empty/retry error fixtures are finite and byte exact', () => {
  assert.deepEqual(UPSTREAM_ERROR_FIXTURES.map((row) => row.id), [
    'json', 'text', 'html', 'empty', 'retry', 'anthropic-standard',
  ]);
  for (const fixture of UPSTREAM_ERROR_FIXTURES.filter((row) => row.body.length > 0 && !row.anthropicError)) {
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

function standardEvidence() {
  const fixture = UPSTREAM_ERROR_FIXTURES.find((row) => row.id === 'anthropic-standard');
  const facts = { ...fixture.anthropicError };
  return { fixture, observations: {
    nativeError: { message: facts.message, details: { upstream_error: { ...facts }, raw_body: fixture.body } },
    durableErrorPayload: { message: facts.message, provider_details: { upstream_error: { ...facts }, raw_body: fixture.body } },
    clientErrors: [{ type: 'error', error: { type: facts.type, message: facts.message }, request_id: facts.request_id }],
  } };
}

test('standard Anthropic error preserves structured facts and exact persisted raw_body independently', () => {
  const { fixture, observations } = standardEvidence();
  assert.deepEqual(JSON.parse(fixture.body), {
    type: 'error', error: { type: fixture.anthropicError.type, message: fixture.anthropicError.message },
    request_id: fixture.anthropicError.request_id,
  });
  assert.doesNotThrow(() => assertUpstreamErrorFidelity(fixture, observations));
  for (const owner of ['nativeError', 'durableErrorPayload', 'clientErrors']) {
    for (const field of ['message', 'type', 'request_id']) {
      const changed = structuredClone(observations);
      const target = owner === 'nativeError' ? changed.nativeError.details.upstream_error
        : owner === 'durableErrorPayload' ? changed.durableErrorPayload.provider_details.upstream_error
          : field === 'request_id' ? changed.clientErrors[0] : changed.clientErrors[0].error;
      target[field] = 'corrupted';
      assert.throws(() => assertUpstreamErrorFidelity(fixture, changed), new RegExp(field, 'u'));
    }
  }
  for (const owner of ['nativeError', 'durableErrorPayload']) {
    const changed = structuredClone(observations);
    (owner === 'nativeError' ? changed.nativeError.details : changed.durableErrorPayload.provider_details).raw_body = fixture.body.trim();
    assert.throws(() => assertUpstreamErrorFidelity(fixture, changed), /raw_body/u);
    const changedMessage = structuredClone(observations);
    changedMessage[owner].message = fixture.body;
    assert.throws(() => assertUpstreamErrorFidelity(fixture, changedMessage), /error message/u);
  }
  const wrongEnvelope = structuredClone(observations);
  wrongEnvelope.clientErrors[0].type = 'message_stop';
  assert.throws(() => assertUpstreamErrorFidelity(fixture, wrongEnvelope), /envelope type/u);
});

test('error inventory retains legacy surface cross-product and adds only standard Anthropic row', () => {
  const legacy = ['json', 'text', 'html', 'empty', 'retry'];
  const surfaces = ['openai-chat-sse', 'anthropic-sse', 'responses-sse', 'responses-websocket'];
  assert.deepEqual(ERROR_SURFACES, surfaces);
  assert.deepEqual(ERROR_FIDELITY_ROWS.map((row) => row.id), [
    ...legacy.flatMap((id) => surfaces.map((surface) => `${id}/${surface}`)),
    'anthropic-standard/anthropic-sse',
  ]);
  assert.doesNotThrow(() => assertErrorFidelityRowInventory(ERROR_FIDELITY_ROWS));
  assert.throws(() => assertErrorFidelityRowInventory(ERROR_FIDELITY_ROWS.slice(1)), /omitted/u);
  assert.throws(() => assertErrorFidelityRowInventory(ERROR_FIDELITY_ROWS.map((row, index) => index === 0 ? { ...row, id: 'wrong-id' } : row)), /row id mismatch/u);
  assert.throws(() => assertErrorFidelityRowInventory([...ERROR_FIDELITY_ROWS, ERROR_FIDELITY_ROWS[0]]), /duplicated/u);
  assert.throws(() => assertErrorFidelityRowInventory([...ERROR_FIDELITY_ROWS, { fixture: 'anthropic-standard', surface: 'responses-sse' }]), /unexpected/u);
});
