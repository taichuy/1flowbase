'use strict';

const crypto = require('node:crypto');

const IGNORED_WIRE_HEADERS = new Set([
  'authorization', 'proxy-authorization', 'x-api-key', 'api-key',
  'host', 'connection', 'proxy-connection', 'keep-alive', 'te', 'trailer',
  'transfer-encoding', 'upgrade', 'content-length', 'accept-encoding',
  'user-agent', 'traceparent', 'tracestate', 'baggage', 'x-request-id',
]);
const COMMA_LIST_HEADERS = new Set(['anthropic-beta']);

function stableValue(value) {
  if (Array.isArray(value)) return value.map(stableValue);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(Object.keys(value).sort().map((key) => [key, stableValue(value[key])]));
}

function normalizedHeaders(headers = {}) {
  const pairs = headers instanceof Headers ? [...headers.entries()] : Object.entries(headers);
  return Object.fromEntries(pairs
    .map(([name, value]) => {
      const normalizedName = name.toLowerCase();
      const values = (Array.isArray(value) ? value : [value]).map(String);
      return [normalizedName, COMMA_LIST_HEADERS.has(normalizedName)
        ? values.flatMap((entry) => entry.split(',').map((token) => token.trim()).filter(Boolean))
        : values];
    })
    .filter(([name]) => !IGNORED_WIRE_HEADERS.has(name) && !name.startsWith('sec-websocket-'))
    .sort(([left], [right]) => left.localeCompare(right)));
}

function normalizedUrl(value) {
  const url = new URL(value, 'http://oracle.invalid');
  const query = [...url.searchParams.entries()].sort(([leftName, leftValue], [rightName, rightValue]) =>
    leftName.localeCompare(rightName) || leftValue.localeCompare(rightValue));
  const encoded = new URLSearchParams(query).toString();
  return `${url.pathname}${encoded ? `?${encoded}` : ''}`;
}

function normalizedRequest(request) {
  return {
    method: String(request.method || 'POST').toUpperCase(),
    url: normalizedUrl(request.url || request.path || '/'),
    headers: normalizedHeaders(request.headers),
    body: stableValue(request.body ?? {}),
  };
}

function normalizedRequestFingerprint(request) {
  return crypto.createHash('sha256')
    .update(JSON.stringify(normalizedRequest(request)))
    .digest('hex');
}

function assertRequestPair({ direct, gateway }) {
  const directFingerprint = normalizedRequestFingerprint(direct);
  const gatewayFingerprint = normalizedRequestFingerprint(gateway);
  if (directFingerprint !== gatewayFingerprint) {
    throw new Error(
      `request fidelity mismatch: direct ${directFingerprint} != gateway ${gatewayFingerprint}`,
    );
  }
  return directFingerprint;
}

const REQUEST_FIDELITY_VECTORS = Object.freeze([
  Object.freeze({
    id: 'openai-chat-typed-and-safe-residual',
    ingress: 'openai_chat',
    expected_upstream_path: '/v1/chat/completions',
    comparison: 'normalized-direct-vs-gateway-sha256',
    request: Object.freeze({
      query: Object.freeze([['fixture_query', 'chat-query-value']]),
      headers: Object.freeze({ 'x-fixture-extension': 'chat-header-value' }),
      body: Object.freeze({
        reasoning_effort: 'high',
        service_tier: 'auto',
        fixture_body_extension: Object.freeze({ nested: 'chat-body-value' }),
      }),
    }),
  }),
  Object.freeze({
    id: 'anthropic-1m-adaptive-effort-context-management',
    ingress: 'anthropic_messages',
    expected_upstream_path: '/v1/messages',
    comparison: 'normalized-direct-vs-gateway-sha256',
    request: Object.freeze({
      query: Object.freeze([['fixture_query', 'anthropic-query-value']]),
      headers: Object.freeze({
        'anthropic-beta': 'context-1m-2025-08-07,fixture-safe-beta',
        'x-fixture-extension': 'anthropic-header-value',
      }),
      body: Object.freeze({
        thinking: Object.freeze({ type: 'adaptive' }),
        output_config: Object.freeze({ effort: 'max' }),
        context_management: Object.freeze({ edits: Object.freeze([{ type: 'clear_tool_uses_20250919' }]) }),
        fixture_body_extension: Object.freeze({ nested: 'anthropic-body-value' }),
      }),
    }),
  }),
  Object.freeze({
    id: 'openai-responses-reasoning-and-safe-residual',
    ingress: 'openai_responses',
    expected_upstream_path: '/v1/responses',
    comparison: 'normalized-direct-vs-gateway-sha256',
    request: Object.freeze({
      query: Object.freeze([['fixture_query', 'responses-query-value']]),
      headers: Object.freeze({ 'x-fixture-extension': 'responses-header-value' }),
      body: Object.freeze({
        reasoning: Object.freeze({ effort: 'high', summary: 'auto' }),
        truncation: 'auto',
        fixture_body_extension: Object.freeze({ nested: 'responses-body-value' }),
      }),
    }),
  }),
]);

const REQUEST_NEGATIVE_VECTORS = Object.freeze([
  Object.freeze({
    id: 'typed-opaque-context-window-conflict', kind: 'typed-opaque-conflict',
    expected: 'fail-before-upstream',
    typed: Object.freeze({ requested_context_window: 1_000_000 }),
    protocol_context: Object.freeze({ source_protocol: 'anthropic_messages', body: Object.freeze({ requested_context_window: 200_000 }) }),
  }),
  Object.freeze({
    id: 'reserved-authorization-residual', kind: 'reserved-field',
    expected: 'fail-before-upstream',
    protocol_context: Object.freeze({ source_protocol: 'openai_chat', headers: Object.freeze({ authorization: Object.freeze(['<forbidden-fixture-value>']) }) }),
  }),
]);

const REQUEST_TRANSLATION_VECTORS = Object.freeze([
  Object.freeze({
    id: 'foreign-protocol-context', kind: 'foreign-protocol',
    expected: 'generate-without-foreign-wire', provider: 'anthropic',
    protocol_context: Object.freeze({
      source_protocol: 'openai_responses',
      body: Object.freeze({ fixture_extension: 'foreign-raw-canary' }),
    }),
    expected_decision: 'omitted_protocol_context_profile_mismatch',
  }),
]);

const EPHEMERAL_NO_LEAK_ORACLE = Object.freeze({
  raw_lifetime: Object.freeze(['initial-invocation', 'tool-callback', 'retry']),
  cleanup: Object.freeze(['terminal-success', 'terminal-failure']),
  forbidden_sinks: Object.freeze(['durable', 'service-log', 'workflow-result', 'wire-audit-artifact']),
  allowed_projection: Object.freeze(['locator', 'digest', 'source_protocol', 'typed_decisions']),
  missing_before_terminal: 'fail-closed',
});

function assertNoEphemeralRawLeak(evidence, canaries) {
  const encoded = JSON.stringify(evidence);
  for (const canary of canaries) {
    if (canary && encoded.includes(canary)) {
      throw new Error('ephemeral raw protocol context leaked into durable evidence');
    }
  }
}

module.exports = {
  EPHEMERAL_NO_LEAK_ORACLE,
  IGNORED_WIRE_HEADERS,
  REQUEST_FIDELITY_VECTORS,
  REQUEST_NEGATIVE_VECTORS,
  REQUEST_TRANSLATION_VECTORS,
  assertNoEphemeralRawLeak,
  assertRequestPair,
  normalizedRequest,
  normalizedRequestFingerprint,
};
