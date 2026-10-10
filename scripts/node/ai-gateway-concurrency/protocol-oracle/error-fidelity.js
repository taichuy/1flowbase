'use strict';

const { isDeepStrictEqual } = require('node:util');

const UPSTREAM_ERROR_FIXTURES = Object.freeze([
  Object.freeze({
    id: 'json', status: 500, contentType: 'application/json',
    body: ' \n{"future_error":{"shape":"unknown"},"message":"keep complete body"}\n ',
    attempts: 1,
  }),
  Object.freeze({
    id: 'text', status: 502, contentType: 'text/plain; charset=utf-8',
    body: ' upstream overloaded: retry later \n', attempts: 1,
  }),
  Object.freeze({
    id: 'html', status: 503, contentType: 'text/html; charset=utf-8',
    body: '<!doctype html><title>fixture unavailable</title>\n<p>preserve &amp; exact</p>\n',
    attempts: 1,
  }),
  Object.freeze({
    id: 'empty', status: 503, contentType: 'application/octet-stream',
    body: '', attempts: 1, emptyPolicy: 'one-shared-non-empty-status-fallback',
  }),
  Object.freeze({
    id: 'retry', status: 429, contentType: 'application/json',
    body: '{"error":"retry fixture","retry_after_ms":1}\n', attempts: 2,
    retry: Object.freeze({ first: 'error', second: 'success' }),
  }),
  Object.freeze({
    id: 'anthropic-standard', status: 400, contentType: 'application/json',
    body: ' \n{"type":"error","error":{"type":"invalid_request_error","message":"Your Claude Code version is too old. Please update."},"request_id":"req_fixture_anthropic_standard"}\n ',
    attempts: 1, surfaces: Object.freeze(['anthropic-sse']),
    anthropicError: Object.freeze({
      type: 'invalid_request_error',
      message: 'Your Claude Code version is too old. Please update.',
      request_id: 'req_fixture_anthropic_standard',
    }),
  }),
]);

const ERROR_SURFACES = Object.freeze([
  'openai-chat-sse', 'anthropic-sse', 'responses-sse', 'responses-websocket',
]);

const ERROR_FIDELITY_ROWS = Object.freeze(UPSTREAM_ERROR_FIXTURES.flatMap((fixture) =>
  (fixture.surfaces ?? ERROR_SURFACES).map((surface) => Object.freeze({
    id: `${fixture.id}/${surface}`, fixture: fixture.id, surface,
  }))));

function assertErrorFidelityRowInventory(rows) {
  const expected = new Set(ERROR_FIDELITY_ROWS.map((row) => row.id));
  const seen = new Set();
  for (const row of rows) {
    const id = `${row.fixture}/${row.surface}`;
    if (row.id !== undefined && row.id !== id) throw new Error(`error fidelity evidence row id mismatch ${row.id}`);
    if (!expected.has(id)) throw new Error(`error fidelity evidence has unexpected row ${id}`);
    if (seen.has(id)) throw new Error(`error fidelity evidence duplicated ${id}`);
    seen.add(id);
  }
  for (const id of expected) {
    if (!seen.has(id)) throw new Error(`error fidelity evidence omitted ${id}`);
  }
}

function upstreamErrorFixture(id) {
  return UPSTREAM_ERROR_FIXTURES.find((fixture) => fixture.id === id) ?? null;
}

function errorFixtureMarker(id) {
  if (!upstreamErrorFixture(id)) throw new Error(`unknown upstream error fixture ${id}`);
  return `1flowbase-upstream-error-fixture:${id}`;
}

function errorFixtureFromBody(body) {
  const encoded = JSON.stringify(body);
  const matches = UPSTREAM_ERROR_FIXTURES.filter((fixture) => encoded.includes(errorFixtureMarker(fixture.id)));
  if (matches.length > 1) throw new Error('mock request contains multiple upstream error fixtures');
  return matches[0] ?? null;
}

function assertUpstreamErrorFidelity(fixture, observations) {
  if (observations.surface === 'responses-websocket') {
    // A non-empty WebSocket frame may explicitly carry an empty message.
    // It is a supplier fact, not an empty HTTP response requiring a fallback.
    const expected = { type: 'mock_upstream_error', message: fixture.body, status: fixture.status, nonce: observations.upstreamNonce };
    const native = observations.nativeError;
    const durable = observations.durableErrorPayload;
    const client = observations.clientResponseErrors;
    if (!observations.upstreamNonce || client?.length !== 1) throw new Error('WebSocket upstream error evidence omitted');
    for (const [label, error] of [
      ['Native', native?.details?.upstream_error],
      ['durable', durable?.provider_details?.upstream_error],
      ['WebSocket client', client[0]],
    ]) {
      if (!isDeepStrictEqual(error, expected)) throw new Error(`${label} WebSocket upstream error object mismatch`);
    }
    const expectedRaw = JSON.stringify({ type: 'error', error: expected });
    for (const [label, raw] of [['Native', native?.details?.raw_body], ['durable', durable?.provider_details?.raw_body]]) {
      if (raw !== expectedRaw) throw new Error(`${label} WebSocket raw_body mismatch`);
    }
    for (const message of [native?.message, durable?.message, ...(observations.clientMessages ?? [])]) {
      if (message !== fixture.body) throw new Error('WebSocket explicit upstream message mismatch');
    }
    if (observations.clientMessages?.length !== 1) throw new Error('WebSocket explicit upstream message omitted');
    return;
  }
  if (observations.surface === 'responses-sse' && fixture.id === 'retry') {
    // The HTTP fixture's upstream error is a JSON scalar. Responses keeps that
    // exact value while Native and durable messages retain the full HTTP body.
    const expected = JSON.parse(fixture.body).error;
    if (!isDeepStrictEqual(observations.clientResponseErrors, [expected])) throw new Error('Responses upstream scalar error mismatch');
    for (const [label, error, raw] of [
      ['Native', observations.nativeError?.details?.upstream_error, observations.nativeError?.details?.raw_body],
      ['durable', observations.durableErrorPayload?.provider_details?.upstream_error, observations.durableErrorPayload?.provider_details?.raw_body],
    ]) {
      if (error !== expected || raw !== fixture.body) throw new Error(`${label} upstream scalar error or raw_body mismatch`);
    }
    if (observations.clientMessages?.length !== 0) throw new Error('Responses scalar error acquired an invented message');
  }
  if (fixture.anthropicError) {
    const expected = fixture.anthropicError;
    const native = observations.nativeError;
    const durable = observations.durableErrorPayload;
    const projections = [
      ['Native', native?.details?.upstream_error],
      ['durable', durable?.provider_details?.upstream_error],
      ...(observations.clientErrors ?? []).map((body, index) => [
        `Anthropic client ${index}`, { ...body?.error, request_id: body?.request_id },
      ]),
    ];
    if (!observations.clientErrors?.length) throw new Error('Anthropic client error evidence omitted');
    for (const [label, error] of projections) {
      for (const field of ['message', 'type', 'request_id']) {
        if (error?.[field] !== expected[field]) throw new Error(`${label} upstream ${field} mismatch`);
      }
    }
    for (const [label, error] of [['Native', native], ['durable', durable]]) {
      if (error?.message !== expected.message) throw new Error(`${label} error message mismatch`);
    }
    for (const [label, rawBody] of [
      ['Native', native?.details?.raw_body], ['durable', durable?.provider_details?.raw_body],
    ]) {
      if (rawBody !== fixture.body) throw new Error(`${label} raw_body did not preserve exact upstream body`);
    }
    for (const body of observations.clientErrors) {
      if (body?.type !== 'error') throw new Error('Anthropic client envelope type mismatch');
    }
    return;
  }
  const values = [observations.nativeMessage, observations.durableMessage, ...(observations.clientMessages ?? [])];
  if (!(observations.surface === 'responses-sse' && fixture.id === 'retry') && !observations.clientMessages?.length) {
    throw new Error('client upstream error message evidence omitted');
  }
  if (fixture.body.length > 0) {
    const labels = ['Native error message', 'durable error message'];
    values.forEach((value, index) => {
      if (value !== fixture.body) {
        throw new Error(`${labels[index] ?? `client error message ${index - 1}`} did not preserve exact upstream body`);
      }
    });
  } else {
    if (values.length < 3 || values.some((value) => typeof value !== 'string' || value.length === 0)) {
      throw new Error('empty-body fallback must be one shared non-empty message');
    }
    if (new Set(values).size !== 1) throw new Error('empty-body fallback diverged across projections');
  }
}

module.exports = {
  ERROR_SURFACES,
  ERROR_FIDELITY_ROWS,
  assertErrorFidelityRowInventory,
  UPSTREAM_ERROR_FIXTURES,
  assertUpstreamErrorFidelity,
  errorFixtureFromBody,
  errorFixtureMarker,
  upstreamErrorFixture,
};
