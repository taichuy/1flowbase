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

function providerTrace(phase, status) {
  return [
    'INFO api_server::provider_runtime: provider runtime operation boundary',
    'operation="invoke_stream_with_live_events"',
    'provider_code=deepseek',
    `installation_id=${INSTALLATION_ID}`,
    `package_sha256=${PACKAGE_SHA256}`,
    `phase="${phase}"`,
    `status="${status}"`,
  ].join(' ');
}

function receipt({ stdout = '', apiOutput = '', selectedInstallation = ASSIGNED } = {}) {
  return buildDiagnosticReceipt({
    details: { stage: 'followup', turn_index: 1, transport_status: 'timed_out' },
    clientResult: { stdout, stderr: 'raw-stderr-secret' },
    apiOutput,
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
    apiOutput: providerTrace('start', 'started'),
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
    apiOutput: providerTrace('start', 'started'),
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

test('Root #1556 F17 malformed, unallowlisted, ambiguous, and missing anchors never become false facts', () => {
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

  const ambiguous = receipt({
    apiOutput: [providerTrace('start', 'started'), providerTrace('start', 'started')].join('\n'),
  });
  assert.equal(ambiguous.correlation.status, 'unknown');
  assert.equal(ambiguous.boundaries.assigned_installation.status, 'observed');
  assert.equal(ambiguous.boundaries.selected_installation.status, 'unknown');
  assert.equal(ambiguous.boundaries.provider_operation.start.status, 'unknown');
});
