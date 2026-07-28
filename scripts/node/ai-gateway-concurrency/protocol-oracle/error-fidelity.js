'use strict';

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
]);

const ERROR_SURFACES = Object.freeze([
  'openai-chat-sse', 'anthropic-sse', 'responses-sse', 'responses-websocket',
]);

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
  const values = [observations.nativeMessage, observations.durableMessage, ...(observations.clientMessages ?? [])];
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
  UPSTREAM_ERROR_FIXTURES,
  assertUpstreamErrorFidelity,
  errorFixtureFromBody,
  errorFixtureMarker,
  upstreamErrorFixture,
};
