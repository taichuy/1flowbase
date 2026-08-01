'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { buildDiagnosticReceipt } = require('../diagnostic-receipt');

const INSTALLATION_ID = '019fbdde-70e8-7862-a2cf-7f9846a4bd1b';
const PACKAGE_SHA256 = 'a'.repeat(64);
const ASSIGNED = Object.freeze({
  provider_code: 'deepseek',
  installation_id: INSTALLATION_ID,
  version: '0.1.18',
  package_sha256: PACKAGE_SHA256,
});

function providerTrace(
  phase,
  status,
  operation = 'invoke_stream_with_live_events',
  installationId = INSTALLATION_ID,
) {
  return [
    'INFO api_server::provider_runtime: provider runtime operation boundary',
    `operation="${operation}"`,
    'provider_code=deepseek',
    `installation_id=${installationId}`,
    `package_sha256=${PACKAGE_SHA256}`,
    `phase="${phase}"`,
    `status="${status}"`,
  ].join(' ');
}

function routeTrace(route) {
  return [
    'INFO api_server::application_public_api::anthropic: anthropic compatible route boundary',
    `route="${route}"`,
    'phase="received"',
  ].join(' ');
}

function routeErrorTrace(statusCode, errorType, suffix = '') {
  return [
    'INFO api_server::application_public_api::anthropic:',
    'anthropic compatible route error boundary',
    `status_code=${statusCode}`,
    `error_type="${errorType}"`,
    suffix,
  ].filter(Boolean).join(' ');
}

function receipt({
  stdout = '', apiOutput = '', apiDeltaStatus = 'observed', selectedInstallation = ASSIGNED,
  transportStatus = 'timed_out',
} = {}) {
  return buildDiagnosticReceipt({
    details: { stage: 'followup', turn_index: 1, transport_status: transportStatus },
    clientResult: { stdout, stderr: 'raw-stderr-secret' },
    apiLogDelta: { status: apiDeltaStatus, output: apiOutput },
    selectedInstallation,
  });
}

test('Root #1556 F17 timeout receipt parses partial Claude events and provider start without raw content', () => {
  const rawSecret = 'raw-secret-canary';
  const result = receipt({
    stdout: [
      JSON.stringify({ type: 'stream_event', event: { type: 'message_start' } }),
      JSON.stringify({
        type: 'stream_event',
        event: { type: 'content_block_delta', delta: { type: 'text_delta', text: rawSecret } },
      }),
      '{malformed-json',
    ].join('\n'),
    apiOutput: [
      routeTrace('messages'),
      providerTrace('start', 'started'),
    ].join('\n'),
  });

  assert.equal(result.stage, 'followup');
  assert.equal(result.boundaries.client_transport.outcome, 'timed_out');
  assert.deepEqual(result.boundaries.claude_tmux.structured_event_types,
    ['message_start', 'content_block_delta']);
  assert.deepEqual(result.boundaries.assigned_installation, {
    status: 'observed',
    ...ASSIGNED,
  });
  assert.deepEqual(result.boundaries.selected_installation, {
    status: 'observed',
    ...ASSIGNED,
  });
  assert.equal(result.boundaries.owned_api_request.status, 'observed');
  assert.equal(result.boundaries.provider_operation.start.status, 'observed');
  assert.equal(result.boundaries.provider_operation.end.status, 'not_observed');
  assert.equal(result.deepest_observed_boundary, 'provider_operation_start');
  assert.doesNotMatch(JSON.stringify(result), /raw-secret-canary|raw-stderr-secret/u);
});

test('Root #1556 F17 receipt recognizes correlated provider end and canonical SSE close', () => {
  const apiOutput = [
    routeTrace('messages'),
    providerTrace('start', 'started'),
    providerTrace('end', 'succeeded'),
    [
      'INFO api_server::compat_sse: compatible public API SSE stream closed',
      'sse_projection=anthropic',
      'terminal_reason=flow_finished',
      'client_disconnected=false',
    ].join(' '),
  ].join('\n');
  const result = receipt({ apiOutput });

  assert.equal(result.boundaries.provider_operation.end.status, 'observed');
  assert.equal(result.boundaries.provider_operation.end.status_value, 'succeeded');
  assert.deepEqual(result.boundaries.canonical_sse_terminal, {
    status: 'observed',
    terminal_reason: 'flow_finished',
  });
  assert.equal(result.boundaries.anthropic_sse_terminal.status, 'not_observed');
  assert.equal(result.deepest_observed_boundary, 'canonical_sse_terminal');
});

test('Root #1556 F17 receipt recognizes Anthropic message_stop without retaining event payloads', () => {
  const result = receipt({
    stdout: JSON.stringify({
      type: 'stream_event',
      event: { type: 'message_stop', secret: 'message-stop-secret' },
    }),
    apiOutput: [routeTrace('messages'), providerTrace('start', 'started')].join('\n'),
  });

  assert.deepEqual(result.boundaries.anthropic_sse_terminal, {
    status: 'observed',
    terminal_event: 'message_stop',
  });
  assert.equal(result.deepest_observed_boundary, 'anthropic_sse_terminal');
  assert.doesNotMatch(JSON.stringify(result), /message-stop-secret/u);
});

test('Root #1556 F17 current assignment alone does not prove runtime selection', () => {
  const result = receipt();

  assert.deepEqual(result.boundaries.assigned_installation, {
    status: 'observed',
    ...ASSIGNED,
  });
  assert.equal(result.correlation.status, 'not_observed');
  assert.equal(result.boundaries.selected_installation.status, 'not_observed');
  assert.equal(result.boundaries.owned_api_request.status, 'not_observed');
  assert.equal(result.boundaries.provider_operation.start.status, 'not_observed');
  assert.equal(result.deepest_observed_boundary, 'client_transport');
});

test('Root #1556 F19 count_tokens route and provider delta remain distinct observed boundaries', () => {
  const result = receipt({
    apiOutput: [
      routeTrace('messages_count_tokens'),
      providerTrace('start', 'started', 'count_tokens'),
      providerTrace('end', 'succeeded', 'count_tokens'),
      'raw unallowlisted secret-canary',
    ].join('\n'),
  });

  assert.deepEqual(result.boundaries.anthropic_route_requests.messages, {
    status: 'not_observed',
  });
  assert.deepEqual(result.boundaries.anthropic_route_requests.messages_count_tokens, {
    status: 'observed', count: 1,
  });
  assert.deepEqual(result.boundaries.owned_api_request, {
    status: 'observed', routes: ['messages_count_tokens'],
  });
  assert.equal(result.boundaries.selected_installation.status, 'observed');
  assert.equal(result.boundaries.provider_operation.operation, 'count_tokens');
  assert.deepEqual(result.boundaries.provider_operation.observed_operations, ['count_tokens']);
  assert.equal(result.boundaries.provider_operation.end.status, 'observed');
  assert.equal(result.deepest_observed_boundary, 'provider_operation_end');
  assert.doesNotMatch(JSON.stringify(result), /secret-canary/u);
});

test('Root #1556 F19 provider sequence selects the last matching operation boundary', () => {
  const result = receipt({
    apiOutput: [
      routeTrace('messages_count_tokens'),
      providerTrace('start', 'started', 'count_tokens'),
      providerTrace('end', 'succeeded', 'count_tokens'),
      routeTrace('messages'),
      providerTrace('start', 'started', 'invoke_stream_with_live_events'),
    ].join('\n'),
  });

  assert.equal(result.correlation.status, 'observed');
  assert.equal(result.boundaries.selected_installation.status, 'observed');
  assert.deepEqual(result.boundaries.provider_operation.observed_operations,
    ['count_tokens', 'invoke_stream_with_live_events']);
  assert.equal(result.boundaries.provider_operation.operation, 'invoke_stream_with_live_events');
  assert.equal(result.boundaries.provider_operation.start.status, 'observed');
  assert.equal(result.boundaries.provider_operation.end.status, 'not_observed');
  assert.equal(result.deepest_observed_boundary, 'provider_operation_start');
});

test('Root #1556 F19 mismatched provider start makes runtime correlation unknown', () => {
  const otherInstallationId = '019fbdde-70e8-7862-a2cf-7f9846a4bd1c';
  const result = receipt({
    apiOutput: [
      providerTrace('start', 'started', 'count_tokens'),
      providerTrace('start', 'started', 'invoke_stream', otherInstallationId),
    ].join('\n'),
  });

  assert.equal(result.correlation.status, 'unknown');
  assert.equal(result.boundaries.selected_installation.status, 'unknown');
  assert.equal(result.boundaries.provider_operation.start.status, 'unknown');
  assert.equal(result.boundaries.provider_operation.end.status, 'unknown');
});

test('Root #1556 F19 duplicate ends after the last start make only the end boundary unknown', () => {
  const result = receipt({
    apiOutput: [
      providerTrace('start', 'started', 'count_tokens'),
      providerTrace('end', 'succeeded', 'count_tokens'),
      providerTrace('end', 'failed', 'count_tokens'),
    ].join('\n'),
  });

  assert.equal(result.correlation.status, 'observed');
  assert.equal(result.boundaries.selected_installation.status, 'observed');
  assert.equal(result.boundaries.provider_operation.start.status, 'observed');
  assert.equal(result.boundaries.provider_operation.end.status, 'unknown');
  assert.equal(result.deepest_observed_boundary, 'provider_operation_start');
});

test('Root #1556 F19 messages route without provider start stops at owned API request', () => {
  const result = receipt({ apiOutput: routeTrace('messages') });

  assert.deepEqual(result.boundaries.anthropic_route_requests.messages, {
    status: 'observed', count: 1,
  });
  assert.equal(result.boundaries.anthropic_route_requests.messages_count_tokens.status,
    'not_observed');
  assert.equal(result.boundaries.owned_api_request.status, 'observed');
  assert.equal(result.boundaries.selected_installation.status, 'not_observed');
  assert.equal(result.boundaries.provider_operation.start.status, 'not_observed');
  assert.equal(result.deepest_observed_boundary, 'owned_api_request');
});

test('Root #1556 F20 repeated route errors aggregate without retaining response messages', () => {
  const secret = 'route-error-message-secret';
  const apiOutput = Array.from({ length: 12 }, () => [
    routeTrace('messages'),
    routeErrorTrace(400, 'invalid_request_error', `message="${secret}"`),
  ]).flat().join('\n');
  const result = receipt({ apiOutput, transportStatus: 'nonzero_exit' });

  assert.equal(result.boundaries.client_transport.outcome, 'nonzero_exit');
  assert.deepEqual(result.boundaries.anthropic_route_requests.messages, {
    status: 'observed', count: 12,
  });
  assert.deepEqual(result.boundaries.anthropic_route_error, {
    status: 'observed',
    status_code: 400,
    error_type: 'invalid_request_error',
    count: 12,
  });
  assert.equal(result.boundaries.selected_installation.status, 'not_observed');
  assert.equal(result.boundaries.provider_operation.start.status, 'not_observed');
  assert.equal(result.deepest_observed_boundary, 'anthropic_route_error');
  assert.doesNotMatch(JSON.stringify(result), /route-error-message-secret|message=/u);
});

test('Root #1556 F20 distinct safe route error pairs make the aggregate unknown', () => {
  const result = receipt({
    apiOutput: [
      routeTrace('messages'),
      routeErrorTrace(422, 'invalid_request_error'),
      routeErrorTrace(401, 'not_authenticated'),
      routeErrorTrace(422, 'invalid_request_error'),
    ].join('\n'),
  });

  assert.deepEqual(result.boundaries.anthropic_route_error, {
    status: 'unknown',
    observed_pairs: [
      { status_code: 401, error_type: 'not_authenticated', count: 1 },
      { status_code: 422, error_type: 'invalid_request_error', count: 2 },
    ],
  });
  assert.equal(result.deepest_observed_boundary, 'owned_api_request');
});

test('Root #1556 F20 malformed route error anchors are ignored without leaking raw fields', () => {
  const secret = 'malformed-route-error-secret';
  const result = receipt({
    apiOutput: [
      routeErrorTrace(399, 'invalid_request_error', `message="${secret}"`),
      routeErrorTrace(500, 'Invalid-Error', `body="${secret}"`),
      `anthropic compatible route error boundary status_code=nope error_type=bad ${secret}`,
    ].join('\n'),
  });

  assert.equal(result.boundaries.anthropic_route_error.status, 'not_observed');
  assert.doesNotMatch(JSON.stringify(result), /malformed-route-error-secret|Invalid-Error/u);
});

test('Root #1556 F19 uncertain log suffix makes all API route and provider facts unknown', () => {
  const result = receipt({
    apiDeltaStatus: 'unknown',
    apiOutput: [routeTrace('messages'), providerTrace('start', 'started')].join('\n'),
  });

  assert.equal(result.boundaries.assigned_installation.status, 'observed');
  assert.equal(result.boundaries.anthropic_route_requests.status, 'unknown');
  assert.equal(result.boundaries.anthropic_route_requests.messages.status, 'unknown');
  assert.equal(result.boundaries.owned_api_request.status, 'unknown');
  assert.equal(result.boundaries.anthropic_route_error.status, 'unknown');
  assert.equal(result.boundaries.selected_installation.status, 'unknown');
  assert.equal(result.boundaries.provider_operation.start.status, 'unknown');
  assert.equal(result.boundaries.provider_operation.end.status, 'unknown');
  assert.equal(result.deepest_observed_boundary, 'client_transport');
});

test('Root #1556 F17 malformed, unallowlisted, and missing anchors never become false facts', () => {
  const rawSecret = 'unallowlisted-secret-canary';
  const malformed = receipt({
    stdout: [
      '{broken',
      JSON.stringify({ type: 'prompt', body: rawSecret }),
      JSON.stringify({ type: 'stream_event', event: { type: 'unknown', body: rawSecret } }),
    ].join('\n'),
    apiOutput: [
      `INFO unallowlisted ${rawSecret}`,
      'provider runtime operation boundary operation="unknown" phase="start" status="started"',
    ].join('\n'),
  });
  assert.equal(malformed.boundaries.claude_tmux.status, 'not_observed');
  assert.equal(malformed.boundaries.assigned_installation.status, 'observed');
  assert.equal(malformed.boundaries.selected_installation.status, 'not_observed');
  assert.equal(malformed.boundaries.owned_api_request.status, 'not_observed');
  assert.equal(malformed.boundaries.provider_operation.start.status, 'not_observed');
  assert.equal(malformed.boundaries.provider_operation.end.status, 'not_observed');
  assert.equal(malformed.boundaries.canonical_sse_terminal.status, 'not_observed');
  assert.equal(malformed.boundaries.anthropic_sse_terminal.status, 'not_observed');
  assert.equal(malformed.deepest_observed_boundary, 'client_transport');
  assert.doesNotMatch(JSON.stringify(malformed), /unallowlisted-secret-canary/u);
});
