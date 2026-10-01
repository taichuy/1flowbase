'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const field = (text, name) => {
  const literal = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return text.match(new RegExp(`(?:^|[\\s{])${literal}=(?:"([^"]+)"|([^\\s}]+))`))?.slice(1).find(Boolean);
};
const time = (value, meta) => Date.parse(value) - Date.parse(meta.startedUtc);
const parse = value => typeof value === 'string' ? JSON.parse(value) : value;

function clientTraces(events, meta) {
  const traces = [];
  let carry = '';
  for (let index = 0; index < events.length; index++) {
    const event = events[index];
    if (event.kind !== 'client/stderr') continue;
    const parts = (carry + event.text).split('\n');
    carry = parts.pop();
    for (let line = 0; line < parts.length; line++) {
      const text = parts[line].replace(/\x1b\[[0-9;]*m/g, '');
      const timestamp = text.match(/^\S+Z/)?.[0];
      const atMs = time(timestamp, meta);
      if (!Number.isFinite(atMs)) continue;
      const sourceEventId = `timeline:${index + 1}:stderr:${line + 1}`;
      const source = { atMs, sourceEventId };
      if (text.includes('codex_otel.agent_communication:')) {
        traces.push({ ...source, kind: 'communication', communicationId: field(text, 'communication_id'),
          state: field(text, 'state'), communicationKind: field(text, 'kind'),
          sender: field(text, 'sender_thread_id'), receiver: field(text, 'receiver_thread_id') });
      }
      if ((field(text, 'thread.id') || field(text, 'thread_id')) !== meta.threadId
          || field(text, 'turn.id') !== meta.mailboxTurnId) continue;
      if (text.includes('codex_api::endpoint::responses_websocket: successfully connected to websocket:'))
        traces.push({ ...source, kind: 'websocket_connected' });
      if (text.includes('codex_api::endpoint::responses_websocket:')
          && text.includes('responses_websocket.stream_request{')
          && !text.includes('responses_websocket.connect{') && !text.includes('thread_spawn{')) {
        if (/responses_websocket: new$/.test(text)) traces.push({ ...source, kind: 'sample_started' });
        if (/responses_websocket: close\s/.test(text)) traces.push({ ...source, kind: 'sample_closed' });
      }
      if (text.includes('post sampling token usage')) traces.push({ ...source, kind: 'post_sampling',
        modelNeedsFollowUp: field(text, 'model_needs_follow_up') === 'true' });
    }
  }
  return traces.sort((a, b) => a.atMs - b.atMs);
}

function samples(traces) {
  const result = [];
  let active;
  for (const trace of traces) {
    if (trace.kind === 'sample_started') active = trace;
    if (trace.kind === 'sample_closed' && active) {
      result.push({ start: active, end: trace });
      active = undefined;
    }
  }
  return result;
}

function mailboxInRequest(body, childPath) {
  if (typeof childPath !== 'string' || !childPath.includes('/')) return false;
  const parentPath = childPath.slice(0, childPath.lastIndexOf('/'));
  const header = `Message Type: FINAL_ANSWER\nTask name: ${parentPath}\nSender: ${childPath}\nPayload:\n`;
  return body?.native_request?.wire_body?.input?.some(item => item.type === 'agent_message'
    && item.author === childPath && item.recipient === parentPath && Array.isArray(item.content)
    && item.content.every(content => content.type === 'input_text' && typeof content.text === 'string')
    && item.content.map(content => content.text).join('\n').startsWith(header)) || false;
}

function buildMailboxEvidence({ events, meta, flows, original }) {
  const traces = clientTraces(events, meta);
  const starts = events.filter(e => e.method === 'item/completed'
    && e.params?.threadId === meta.threadId && e.params.turnId === meta.mailboxTurnId
    && e.params.item?.type === 'subAgentActivity' && e.params.item.kind === 'started');
  const child = starts[0]?.params.item.agentThreadId;
  const childPath = starts[0]?.params.item.agentPath;
  const completed = events.find(e => e.method === 'item/completed'
    && e.params?.threadId === meta.threadId && e.params.turnId === meta.mailboxTurnId
    && e.params.item?.type === 'subAgentActivity' && e.params.item.kind === 'completed'
    && e.params.item.agentThreadId === child);
  const terminal = events.find(e => e.method === 'turn/completed'
    && e.params?.threadId === meta.threadId && e.params.turn?.id === meta.mailboxTurnId
    && e.params.turn.status === 'completed');
  const diagnostics = [];
  if (starts.length !== 1 || !completed || !terminal) diagnostics.push('missing correlated child completion or parent terminal');
  const send = traces.find(t => t.kind === 'communication' && t.state === 'send'
    && t.communicationKind === 'result' && t.sender === child && t.receiver === meta.threadId);
  const delivery = traces.find(t => t.kind === 'communication' && t.state === 'receive'
    && t.communicationId && t.communicationId === send?.communicationId);
  if (!send || !delivery) diagnostics.push('missing actual result send/receive communication identity');
  const rootFlows = flows.filter(f => f.clientThreadId === meta.threadId
    && f.logContext?.turn_id === meta.mailboxTurnId);
  const calls = original.filter(e => e.eventType === 'provider_semantic_step'
    && e.originalPayload?.kind === 'model_call' && rootFlows.some(f => f.flowRunId === e.flowRunId))
    .map(e => {
      let body;
      try { body = parse(e.originalPayload.body); } catch { return null; }
      if (!body?.native_request?.wire_body || body.native_request.wire_body.generate === false) return null;
      return { ...e, body, atMs: time(e.createdAt, meta) };
    }).filter(Boolean);
  const allSamples = samples(traces);
  const candidates = allSamples.filter(sample => sample.start.atMs < (delivery?.atMs ?? -Infinity)
    && sample.end.atMs >= (delivery?.atMs ?? Infinity));
  const candidateChecks = [];
  let evidence;
  for (const before of candidates) {
    const nextStart = allSamples.find(s => s.start.atMs > before.end.atMs);
    const post = traces.find(t => t.kind === 'post_sampling' && t.atMs >= before.end.atMs
      && t.atMs < (nextStart?.start.atMs ?? Infinity) && t.modelNeedsFollowUp);
    const beforeCalls = calls.filter(c => c.atMs >= before.start.atMs && c.atMs <= before.end.atMs);
    const modelTools = events.filter(e => e.method === 'item/started'
      && e.params?.threadId === meta.threadId && e.params.turnId === meta.mailboxTurnId
      && ['commandExecution', 'mcpToolCall', 'dynamicToolCall', 'webSearch', 'fileChange'].includes(e.params.item?.type)
      && e.atMs >= before.start.atMs && e.atMs <= (post?.atMs ?? before.end.atMs));
    const reasoning = events.find(e => e.method === 'item/completed'
      && e.params?.threadId === meta.threadId && e.params.turnId === meta.mailboxTurnId
      && e.params.item?.type === 'reasoning' && e.atMs >= (delivery?.atMs ?? Infinity)
      && e.atMs <= (post?.atMs ?? -Infinity));
    const check = { startAtMs: before.start.atMs, endAtMs: before.end.atMs,
      postSampling: Boolean(post), reasoning: Boolean(reasoning), tools: modelTools.length,
      beforeCalls: beforeCalls.length, successorSample: Boolean(nextStart) };
    candidateChecks.push(check);
    if (!post || !reasoning || modelTools.length || beforeCalls.length !== 1 || !nextStart) continue;
    const connected = traces.find(t => t.kind === 'websocket_connected'
      && t.atMs > before.end.atMs && t.atMs <= nextStart.end.atMs);
    const afterCalls = calls.filter(c => c.atMs >= nextStart.start.atMs && c.atMs <= nextStart.end.atMs
      && mailboxInRequest(c.body, childPath));
    check.connected = Boolean(connected);
    check.mailboxCalls = afterCalls.length;
    if (!connected || afterCalls.length !== 1 || afterCalls[0].flowRunId === beforeCalls[0].flowRunId) continue;
    if (events.some(e => e.method === 'error' && e.params?.threadId === meta.threadId
        && e.params.turnId === meta.mailboxTurnId && e.atMs >= before.start.atMs
        && e.atMs <= nextStart.end.atMs)) continue;
    evidence = { before, after: nextStart, post, connected, beforeCall: beforeCalls[0], afterCall: afterCalls[0] };
    break;
  }
  if (!evidence) diagnostics.push('no source-backed active sampling preemption and successor WS request with mailbox input');
  if (diagnostics.length) return { status: 'UNVERIFIED', diagnostics, records: [], counts: {
    traces: traces.length, calls: calls.length, samples: allSamples.length,
    deliveryAtMs: delivery?.atMs ?? null, preemptionCandidates: candidates.length,
    candidateChecks,
    sampleWindows: allSamples.map(s => ({ startAtMs: s.start.atMs, endAtMs: s.end.atMs })),
    callWindows: calls.map(c => ({ flowRunId: c.flowRunId, atMs: c.atMs,
      mailboxInputPresent: mailboxInRequest(c.body, childPath),
    })),
  } };
  const { before, after, post, connected, beforeCall, afterCall } = evidence;
  if (!(afterCall.atMs < terminal.atMs)) return { status: 'UNVERIFIED', diagnostics: ['successor request did not precede parent terminal'], records: [] };
  const common = { rootThreadId: meta.threadId, parentTurnId: meta.mailboxTurnId, childThreadId: child, rolloutId: meta.threadId };
  const records = [
    { ...common, kind: 'parent/native-sampling-before', flowRunId: beforeCall.flowRunId,
      parentSampleId: before.start.sourceEventId, sourceEventId: beforeCall.id, atMs: beforeCall.atMs },
    { ...common, kind: 'mailbox/child-completed', sourceEventId: delivery.sourceEventId,
      communicationId: delivery.communicationId, sendSourceEventId: send.sourceEventId, atMs: delivery.atMs },
    { ...common, kind: 'gateway/websocket-reconnected', previousFlowRunId: beforeCall.flowRunId,
      flowRunId: afterCall.flowRunId, sourceEventId: afterCall.id, atMs: connected.atMs,
      clientCarrierSourceEventId: connected.sourceEventId, preemptionSourceEventId: post.sourceEventId,
      originalPayload: { flow_run_id: afterCall.flowRunId, payload: {
        kind: afterCall.originalPayload.kind, trigger_request_id: afterCall.originalPayload.trigger_request_id,
        context_response_id: afterCall.originalPayload.context_response_id,
        native_request_digest: afterCall.body.native_request.digest,
        mailbox_input_present: true,
      } } },
    { ...common, kind: 'parent/native-sampling-after', flowRunId: afterCall.flowRunId,
      parentSampleId: after.start.sourceEventId, sourceEventId: afterCall.id, atMs: afterCall.atMs },
  ];
  if (records.some((r, i) => i > 0 && r.atMs <= records[i - 1].atMs))
    return { status: 'UNVERIFIED', diagnostics: ['cross-source monotonic order is not established'], records: [] };
  return { status: 'COLLECTED', diagnostics: [], records };
}

function databaseRows(meta, repositoryRoot, childThreadId) {
  for (const value of [meta.appId, meta.threadId, meta.mailboxTurnId, childThreadId])
    if (!UUID.test(value || '')) throw new Error('Invalid DB evidence selector');
  const { parseEnvFile } = require(path.join(repositoryRoot, 'scripts/node/dev-up/env.js'));
  const config = parseEnvFile(path.join(repositoryRoot, 'api/apps/api-server/.env'));
  const url = new URL(process.env.RLS_DATABASE_URL || config.API_DATABASE_URL);
  const env = { ...process.env, PGHOST: url.hostname, PGPORT: url.port || '5432',
    PGDATABASE: decodeURIComponent(url.pathname.slice(1)), PGUSER: decodeURIComponent(url.username),
    PGPASSWORD: decodeURIComponent(url.password),
    PGOPTIONS: '-c default_transaction_read_only=on -c statement_timeout=10000' };
  if (url.searchParams.has('sslmode')) env.PGSSLMODE = url.searchParams.get('sslmode');
  const execute = sql => {
    const result = spawnSync('psql', ['-X', '-A', '-t', '-c', sql], {
      env, encoding: 'utf8', timeout: 15000, maxBuffer: 32 * 1024 * 1024,
    });
    if (result.status !== 0) throw new Error('Read-only DB evidence query failed');
    return JSON.parse(result.stdout.trim());
  };
  const flows = execute(`SELECT coalesce(json_agg(q),'[]'::json) FROM (
    SELECT f.id AS "flowRunId",f.started_at AS "startedAt",f.finished_at AS "finishedAt",
      f.log_context AS "logContext",c.client_thread_id AS "clientThreadId"
    FROM flow_runs f JOIN application_conversations c ON c.id=(f.log_context->>'log_conversation_id')::uuid
    WHERE f.application_id='${meta.appId}'::uuid AND c.client_thread_id='${meta.threadId}'
      AND f.log_context->>'turn_id'='${meta.mailboxTurnId}' ORDER BY f.started_at) q;`);
  if (!flows.length) return { flows, original: [] };
  for (const f of flows) if (!UUID.test(f.flowRunId)) throw new Error('Invalid stored flow selector');
  const original = execute(`SELECT coalesce(json_agg(q),'[]'::json) FROM (
    SELECT e.id,e.sequence,e.created_at AS "createdAt",e.event_type AS "eventType",e.flow_run_id AS "flowRunId",
      runtime_event_original_payload(e.payload,e.raw_json_payloads,e.flow_run_id) AS "originalPayload"
    FROM runtime_events e WHERE e.flow_run_id IN (${flows.map(f => `'${f.flowRunId}'::uuid`).join(',')})
      AND e.event_type='provider_semantic_step' ORDER BY e.created_at,e.sequence) q;`);
  return { flows, original };
}

async function collectMailboxEvidence({ events, meta, out, repositoryRoot }) {
  let result;
  try {
    const child = events.find(e => e.method === 'item/completed' && e.params?.threadId === meta.threadId
      && e.params.turnId === meta.mailboxTurnId && e.params.item?.type === 'subAgentActivity'
      && e.params.item.kind === 'started')?.params.item.agentThreadId;
    const rows = databaseRows(meta, repositoryRoot, child);
    result = buildMailboxEvidence({ events, meta, ...rows });
    fs.writeFileSync(path.join(out, 'mailbox-source-index.json'), JSON.stringify({
      flows: rows.flows.map(f => ({ flowRunId: f.flowRunId, startedAt: f.startedAt,
        finishedAt: f.finishedAt, clientThreadId: f.clientThreadId, turnId: f.logContext.turn_id })),
      eventIds: rows.original.map(e => ({ id: e.id, flowRunId: e.flowRunId,
        createdAt: e.createdAt, kind: e.originalPayload?.kind })),
    }), { mode: 0o600 });
  } catch {
    result = { status: 'UNVERIFIED', diagnostics: ['mailbox source collection failed; no synthetic evidence emitted'], records: [] };
  }
  const diagnosticPath = path.join(out, 'mailbox-evidence-diagnostics.json');
  fs.writeFileSync(diagnosticPath, JSON.stringify({ status: result.status,
    diagnostics: result.diagnostics, counts: result.counts || null }), { mode: 0o600 });
  if (result.status !== 'COLLECTED') return { status: result.status, diagnosticPath, diagnostics: result.diagnostics };
  const rawLogPath = path.join(out, 'mailbox-evidence.jsonl');
  fs.writeFileSync(rawLogPath, `${result.records.map(r => JSON.stringify(r)).join('\n')}\n`, { mode: 0o600 });
  return { status: 'COLLECTED', rawLogPath, diagnosticPath, diagnostics: [] };
}

module.exports = { collectMailboxEvidence, buildMailboxEvidence, clientTraces, databaseRows };
