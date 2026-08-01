'use strict';

const RECEIPT_SCHEMA = '1flowbase.local-count-tokens-diagnostic-receipt/v1';
const CLIENT_EVENT_TYPES = new Set([
  'assistant',
  'content_block_delta',
  'content_block_start',
  'message_delta',
  'message_start',
  'message_stop',
  'result',
]);
const PROVIDER_OPERATIONS = new Set([
  'count_tokens', 'invoke_stream', 'invoke_stream_with_live_events',
]);
const PROVIDER_PHASES = new Set(['start', 'end']);
const PROVIDER_STATUSES = new Set(['started', 'succeeded', 'failed']);
const TRANSPORT_STATUSES = new Set([
  'assistant_missing', 'execution_error', 'nonzero_exit', 'setup_error', 'terminal_missing', 'timed_out',
]);
const CANONICAL_TERMINALS = new Set([
  'flow_cancelled', 'flow_failed', 'flow_finished', 'flow_incomplete', 'waiting_callback', 'waiting_human',
]);
const ANTHROPIC_ROUTES = new Set(['messages', 'messages_count_tokens']);
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;

function observed(fields = {}) {
  return { status: 'observed', ...fields };
}

function missing(status = 'not_observed') {
  return { status };
}

function structuredClientEvents(stdout) {
  const eventTypes = [];
  for (const line of String(stdout || '').split(/\r?\n/u)) {
    const trimmed = line.replace(/\u001b\[[0-?]*[ -/]*[@-~]/gu, '').trim();
    if (!trimmed.startsWith('{')) continue;
    let event;
    try { event = JSON.parse(trimmed); } catch { continue; }
    const eventType = event?.type === 'stream_event' ? event.event?.type : event?.type;
    if (CLIENT_EVENT_TYPES.has(eventType)) eventTypes.push(eventType);
  }
  return [...new Set(eventTypes)];
}

function traceField(line, name) {
  const match = String(line).match(new RegExp(`(?:^|\\s)${name}=(?:"([^"]*)"|([^\\s]+))`, 'u'));
  return match?.[1] ?? match?.[2] ?? null;
}

function providerTrace(line, index) {
  if (!line.includes('provider runtime operation boundary')) return null;
  const value = {
    index,
    operation: traceField(line, 'operation'),
    provider_code: traceField(line, 'provider_code'),
    installation_id: traceField(line, 'installation_id'),
    package_sha256: traceField(line, 'package_sha256'),
    phase: traceField(line, 'phase'),
    status: traceField(line, 'status'),
  };
  if (!PROVIDER_OPERATIONS.has(value.operation)
    || !/^[a-z0-9_-]{1,64}$/u.test(value.provider_code || '')
    || !UUID_PATTERN.test(value.installation_id || '')
    || !SHA256_PATTERN.test(value.package_sha256 || '')
    || !PROVIDER_PHASES.has(value.phase)
    || !PROVIDER_STATUSES.has(value.status)) return null;
  if ((value.phase === 'start') !== (value.status === 'started')) return null;
  return value;
}

function canonicalClose(line, index) {
  if (!line.includes('compatible public API SSE stream closed')) return null;
  const projection = traceField(line, 'sse_projection');
  const terminalReason = traceField(line, 'terminal_reason');
  const clientDisconnected = traceField(line, 'client_disconnected');
  if (projection !== 'anthropic' || clientDisconnected !== 'false'
    || !CANONICAL_TERMINALS.has(terminalReason)) return null;
  return { index, terminal_reason: terminalReason };
}

function anthropicRoute(line) {
  if (!line.includes('anthropic compatible route boundary')) return null;
  const route = traceField(line, 'route');
  const phase = traceField(line, 'phase');
  if (!ANTHROPIC_ROUTES.has(route) || phase !== 'received') return null;
  return route;
}

function observedRouteCount(count) {
  return count > 0 ? observed({ count }) : missing();
}

function routeBoundaries(apiOutput) {
  const counts = { messages: 0, messages_count_tokens: 0 };
  for (const line of String(apiOutput || '').split(/\r?\n/u)) {
    const route = anthropicRoute(line.replace(/\u001b\[[0-?]*[ -/]*[@-~]/gu, ''));
    if (route) counts[route] += 1;
  }
  const routes = Object.entries(counts).filter(([, count]) => count > 0).map(([route]) => route);
  return {
    anthropic_route_requests: {
      status: routes.length ? 'observed' : 'not_observed',
      messages: observedRouteCount(counts.messages),
      messages_count_tokens: observedRouteCount(counts.messages_count_tokens),
    },
    owned_api_request: routes.length ? observed({ routes }) : missing(),
  };
}

function correlatedApiBoundaries(apiOutput, assigned) {
  const lines = String(apiOutput || '').split(/\r?\n/u)
    .map((line) => line.replace(/\u001b\[[0-?]*[ -/]*[@-~]/gu, ''));
  const traces = lines.map(providerTrace).filter(Boolean).filter((trace) => (
    trace.provider_code === assigned.provider_code
      && trace.installation_id === assigned.installation_id
      && trace.package_sha256 === assigned.package_sha256
  ));
  const starts = traces.filter((trace) => trace.phase === 'start');
  if (starts.length === 0) {
    return {
      correlation: missing(),
      provider_operation: { start: missing(), end: missing() },
      canonical_sse_terminal: missing(),
    };
  }
  if (starts.length !== 1) {
    return {
      correlation: missing('unknown'),
      provider_operation: { start: missing('unknown'), end: missing('unknown') },
      canonical_sse_terminal: missing('unknown'),
    };
  }

  const start = starts[0];
  const ends = traces.filter((trace) => trace.phase === 'end'
    && trace.operation === start.operation && trace.index > start.index);
  const closes = lines.map(canonicalClose).filter(Boolean)
    .filter((close) => close.index > start.index);
  return {
    correlation: observed({ method: 'assigned_installation_identity' }),
    provider_operation: {
      operation: start.operation,
      start: observed({ status_value: start.status }),
      end: ends.length === 1
        ? observed({ status_value: ends[0].status })
        : missing(ends.length > 1 ? 'unknown' : 'not_observed'),
    },
    canonical_sse_terminal: closes.length === 1
      ? observed({ terminal_reason: closes[0].terminal_reason })
      : missing(closes.length > 1 ? 'unknown' : 'not_observed'),
  };
}

function deepestBoundary(boundaries) {
  if (boundaries.anthropic_sse_terminal.status === 'observed') return 'anthropic_sse_terminal';
  if (boundaries.canonical_sse_terminal.status === 'observed') return 'canonical_sse_terminal';
  if (boundaries.provider_operation.end.status === 'observed') return 'provider_operation_end';
  if (boundaries.provider_operation.start.status === 'observed') return 'provider_operation_start';
  if (boundaries.selected_installation.status === 'observed') return 'selected_installation';
  if (boundaries.owned_api_request.status === 'observed') return 'owned_api_request';
  if (boundaries.client_transport.status === 'observed') return 'client_transport';
  if (boundaries.claude_tmux.status === 'observed') return 'claude_tmux';
  return 'not_observed';
}

function buildDiagnosticReceipt({
  details, clientResult, apiLogDelta, selectedInstallation: assignedInstallation,
}) {
  const assigned = assignedInstallation
    && /^[a-z0-9_-]{1,64}$/u.test(assignedInstallation.provider_code || '')
    && UUID_PATTERN.test(assignedInstallation.installation_id || '')
    && SHA256_PATTERN.test(assignedInstallation.package_sha256 || '')
    && /^[0-9A-Za-z.+-]{1,64}$/u.test(assignedInstallation.version || '')
    ? {
      provider_code: assignedInstallation.provider_code,
      installation_id: assignedInstallation.installation_id,
      version: assignedInstallation.version,
      package_sha256: assignedInstallation.package_sha256,
    }
    : null;
  const clientEvents = structuredClientEvents(clientResult?.stdout);
  const deltaObserved = apiLogDelta?.status === 'observed'
    && typeof apiLogDelta.output === 'string';
  const routeEvidence = deltaObserved
    ? routeBoundaries(apiLogDelta.output)
    : {
      anthropic_route_requests: {
        status: 'unknown',
        messages: missing('unknown'),
        messages_count_tokens: missing('unknown'),
      },
      owned_api_request: missing('unknown'),
    };
  const apiBoundaries = deltaObserved && assigned
    ? correlatedApiBoundaries(apiLogDelta.output, assigned)
    : {
      correlation: missing('unknown'),
      provider_operation: { start: missing('unknown'), end: missing('unknown') },
      canonical_sse_terminal: missing('unknown'),
    };
  const boundaries = {
    claude_tmux: clientEvents.length
      ? observed({ structured_event_types: clientEvents })
      : missing(),
    client_transport: TRANSPORT_STATUSES.has(details?.transport_status)
      ? observed({ outcome: details.transport_status })
      : missing('unknown'),
    assigned_installation: assigned ? observed(assigned) : missing('unknown'),
    selected_installation: apiBoundaries.correlation.status === 'observed'
      ? observed(assigned)
      : missing(apiBoundaries.correlation.status),
    anthropic_route_requests: routeEvidence.anthropic_route_requests,
    owned_api_request: routeEvidence.owned_api_request,
    provider_operation: apiBoundaries.provider_operation,
    canonical_sse_terminal: apiBoundaries.canonical_sse_terminal,
    anthropic_sse_terminal: clientEvents.includes('message_stop')
      ? observed({ terminal_event: 'message_stop' })
      : missing(),
  };
  return {
    schema_version: RECEIPT_SCHEMA,
    stage: details?.stage === 'followup' ? 'followup' : 'unknown',
    turn_index: Number.isInteger(details?.turn_index) ? details.turn_index : null,
    correlation: apiBoundaries.correlation,
    deepest_observed_boundary: deepestBoundary(boundaries),
    boundaries,
  };
}

module.exports = { RECEIPT_SCHEMA, buildDiagnosticReceipt };
