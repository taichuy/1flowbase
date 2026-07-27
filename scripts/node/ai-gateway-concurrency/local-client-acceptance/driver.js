'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { writeArtifact, createTimeline } = require('./artifacts');
const {
  ARTIFACT_SCHEMA,
  CLIENT_PROTOCOLS,
  TOOL_ASSETS,
  VECTOR_MANIFEST,
  buildClientPlan,
  selectExecutionSurface,
  vectorsFor,
} = require('./contract');
const { discoverClients, findExecutable, probeVersion } = require('./discovery');
const durableEvidence = require('./durable');
const { OwnedResources, executeTmux } = require('./lifecycle');

const TOOL_ITEM_TYPES = Object.freeze(['command_execution', 'mcp_tool_call', 'tool_call']);

function clientPaths(root, client) {
  const clientRoot = path.join(root, client);
  const result = {
    root: clientRoot,
    config: path.join(clientRoot, 'config'),
    output: path.join(clientRoot, 'output'),
    toolAssets: {},
  };
  fs.mkdirSync(result.config, { recursive: true, mode: 0o700 });
  fs.mkdirSync(result.output, { recursive: true, mode: 0o700 });
  for (const [placeholder, asset] of Object.entries(TOOL_ASSETS)) {
    const filePath = path.join(result.output, asset.filename);
    fs.writeFileSync(filePath, asset.content, { mode: 0o600 });
    result.toolAssets[placeholder] = filePath;
  }
  result.toolFile = result.toolAssets.TOOL_PATH;
  return result;
}

function writeConfigs(plan) {
  for (const config of plan.configFiles) {
    fs.mkdirSync(path.dirname(config.path), { recursive: true, mode: 0o700 });
    fs.writeFileSync(config.path, config.content, { mode: 0o600 });
  }
}

function publicEnvironment(environment) {
  return Object.fromEntries(Object.keys(environment).sort().map((name) => {
    if (name === 'OPENCODE_CONFIG_CONTENT') return [name, '<isolated-config>'];
    if (/(?:api_?key|token|secret|credential|authorization)/iu.test(name)) {
      return [name, environment[name] ? '<redacted>' : environment[name]];
    }
    return [name, environment[name]];
  }));
}

function publicPlan(plan, surface) {
  return {
    surface,
    client_surface: plan.client_surface,
    executable: plan.invocation.executable,
    cwd: plan.invocation.cwd,
    args: plan.invocation.args,
    environment: publicEnvironment(plan.environment),
    generated_config_paths: plan.configFiles.map((config) => config.path),
    ...(plan.protocol_profile ? { protocol_profile: plan.protocol_profile } : {}),
  };
}

function publicExpectation(expected) {
  return {
    ...expected,
    ...(expected.assistant_texts ? {
      assistant_texts: expected.assistant_texts.map((value) => ({
        sha256: crypto.createHash('sha256').update(value).digest('hex'),
        utf8_bytes: Buffer.byteLength(value),
      })),
    } : {}),
  };
}

function overallStatus(clients, cleanupErrors) {
  if (cleanupErrors.length || clients.some((client) => client.status === 'fail')) return 'fail';
  if (clients.every((client) => client.status === 'skipped')) return 'skipped';
  if (clients.some((client) => client.status === 'skipped')) return 'partial';
  return 'pass';
}

function includesMarker(value, marker) {
  if (typeof value === 'string') return value.includes(marker);
  if (Array.isArray(value)) return value.some((item) => includesMarker(item, marker));
  if (value && typeof value === 'object') return Object.values(value).some((item) => includesMarker(item, marker));
  return false;
}

function structuredEvents(output) {
  return String(output).split(/\r?\n/u).flatMap((line) => {
    const trimmed = line.replace(/\u001b\[[0-?]*[ -/]*[@-~]/gu, '').trim();
    if (!trimmed.startsWith('{')) return [];
    try { return [JSON.parse(trimmed)]; } catch { return []; }
  });
}

function assistantTextValues(client, event) {
  if (client === 'codex') {
    return event.type === 'item.completed' && event.item?.type === 'agent_message'
      ? [event.item.text ?? event.item.content].filter((value) => typeof value === 'string')
      : [];
  }
  if (client === 'claude') {
    if (event.type === 'assistant') {
      return (event.message?.content ?? [])
        .filter((part) => part.type === 'text' && typeof part.text === 'string')
        .map((part) => part.text);
    }
    return event.type === 'result' && typeof event.result === 'string' ? [event.result] : [];
  }
  if (event.type === 'message.part.updated' && event.properties?.part?.type === 'text') {
    return [event.properties.part.text].filter((value) => typeof value === 'string');
  }
  if (event.type === 'text' && event.part?.type === 'text') {
    return [event.part.text].filter((value) => typeof value === 'string');
  }
  return [];
}

function toolCalls(client, event) {
  if (client === 'codex') {
    if (!TOOL_ITEM_TYPES.includes(event.item?.type)) return [];
    return [{
      id: event.item.call_id || event.item.id || JSON.stringify(event.item),
      value: event.item,
    }];
  }
  if (client === 'claude') {
    if (event.type !== 'assistant') return [];
    return (event.message?.content ?? []).filter((part) => part.type === 'tool_use').map((part) => ({
      id: part.id || `${part.name}:${JSON.stringify(part.input)}`,
      value: part,
    }));
  }
  const part = event.type === 'message.part.updated'
    ? event.properties?.part
    : event.type === 'tool_use' ? event.part : null;
  if (!part || !['tool', 'tool_call'].includes(part.type)) return [];
  return [{
    id: part.callID || part.callId || part.id || `${part.tool}:${JSON.stringify(part.state?.input)}`,
    value: part,
  }];
}

function isToolResult(client, event, marker) {
  if (!includesMarker(event, marker)) return false;
  if (client === 'codex') return TOOL_ITEM_TYPES.includes(event.item?.type);
  if (client === 'claude') {
    return event.type === 'user'
      || event.message?.content?.some?.((part) => part.type === 'tool_result');
  }
  const part = event.type === 'message.part.updated'
    ? event.properties?.part
    : event.type === 'tool_use' ? event.part : null;
  return ['tool', 'tool_result'].includes(part?.type) || event.type === 'tool_use';
}

function orderedMarkerText(value, markers) {
  let cursor = 0;
  for (const marker of markers) {
    const index = value.indexOf(marker, cursor);
    if (index === -1) return false;
    cursor = index + marker.length;
  }
  return true;
}

function assistantTextIndices(client, events, expectedTexts) {
  let cursor = 0;
  const indices = [];
  for (const expected of expectedTexts) {
    const relative = events.slice(cursor).findIndex((event) => (
      assistantTextValues(client, event).some((text) => text === expected)
    ));
    if (relative === -1) return [];
    const index = cursor + relative;
    indices.push(index);
    cursor = index + 1;
  }
  return indices;
}

function evaluateToolAttempt(client, vector, events) {
  const calls = events.flatMap((event, index) => (
    toolCalls(client, event).map((call) => ({ ...call, index }))
  ));
  const callsById = new Map();
  for (const call of calls) {
    if (!callsById.has(call.id)) callsById.set(call.id, call);
  }
  const distinctCalls = [...callsById.values()];
  const resultIndices = vector.expected.tool_result_markers.map((marker) => (
    events.findIndex((event) => isToolResult(client, event, marker))
  ));
  const finalIndex = events.findIndex((event) => (
    assistantTextValues(client, event).some((text) => text === vector.expected.assistant_texts.at(-1))
  ));
  let chronology = false;
  if (vector.expected.tool_mode === 'parallel_one_callback_task') {
    chronology = resultIndices.every((index) => index >= 0)
      && distinctCalls.length >= vector.expected.minimum_tool_calls
      && distinctCalls.slice(0, vector.expected.minimum_tool_calls)
        .every((call) => call.index <= Math.min(...resultIndices));
  } else if (vector.expected.tool_mode === 'sequential_callback_tasks_one_turn') {
    const [firstResult, secondResult] = resultIndices;
    chronology = firstResult >= 0 && secondResult > firstResult
      && distinctCalls.some((call) => call.index < firstResult)
      && distinctCalls.some((call) => call.index > firstResult && call.index < secondResult);
  } else {
    chronology = resultIndices[0] >= 0 && distinctCalls.some((call) => call.index <= resultIndices[0]);
  }
  const resultsBeforeFinal = resultIndices.length > 0
    && resultIndices.every((index) => index >= 0 && finalIndex > index);
  const pass = chronology && resultsBeforeFinal;
  const observed = [];
  if (distinctCalls.length >= vector.expected.minimum_tool_calls) observed.push('tool_calls_observed');
  if (resultIndices.every((index) => index >= 0)) observed.push('tool_results_observed');
  if (chronology) observed.push(`${vector.expected.tool_mode}_observed`);
  if (resultsBeforeFinal) observed.push('final_marker_observed');
  return { pass, reason: pass ? null : 'tool_callback_evidence_missing', observed_events: observed };
}

function evaluateAttempt(client, vector, result, protocol = null) {
  if (result.timed_out) return { pass: false, reason: 'client_process_timed_out', observed_events: [] };
  const combined = `${result.stdout || ''}\n${result.stderr || ''}`;
  if (vector.expected.exit === 'failure') {
    const markers = vector.expected.error_markers ?? [];
    const visible = result.exit_code !== 0 && orderedMarkerText(combined, markers);
    return {
      pass: visible,
      reason: visible ? null : 'provider_error_body_missing',
      observed_events: visible ? ['provider_error_body_observed'] : [],
    };
  }
  if (result.exit_code !== 0) {
    return { pass: false, reason: 'client_process_failed', observed_events: [] };
  }
  if (client === 'codex' && protocol === 'responses_websocket') {
    if (/falling back to HTTP|fallback_to_http/iu.test(combined)) {
      return { pass: false, reason: 'responses_websocket_http_fallback', observed_events: [] };
    }
  }
  const events = structuredEvents(result.stdout || '');
  if (vector.kind === 'tools') return evaluateToolAttempt(client, vector, events);
  const indices = assistantTextIndices(client, events, vector.expected.assistant_texts ?? []);
  const observed = indices.length === vector.expected.assistant_texts?.length;
  return {
    pass: observed,
    reason: observed ? null : 'ordered_assistant_text_missing',
    observed_events: observed ? ['ordered_assistant_text_observed'] : [],
  };
}

function continuationId(client, result, existing = null) {
  if (client === 'claude') return existing;
  const events = structuredEvents(result.stdout || '');
  if (client === 'codex') {
    return events.find((event) => event.type === 'thread.started')?.thread_id ?? null;
  }
  return events.find((event) => typeof event.sessionID === 'string')?.sessionID
    ?? events.find((event) => typeof event.properties?.sessionID === 'string')?.properties.sessionID
    ?? null;
}

function mergedTurnResult(turns) {
  const failed = turns.find((turn) => turn.result.exit_code !== 0 || turn.result.timed_out);
  return {
    exit_code: failed ? failed.result.exit_code : 0,
    signal: failed?.result.signal ?? null,
    timed_out: turns.some((turn) => turn.result.timed_out),
    stdout: turns.map((turn) => turn.result.stdout || '').join('\n'),
    stderr: turns.map((turn) => turn.result.stderr || '').join('\n'),
    turns,
  };
}

async function executeVector({
  client,
  found,
  target,
  paths,
  vector,
  protocol,
  executor,
  executionOptions,
  surface,
  secrets,
  timeline,
}) {
  let sessionId = vector.turns.length > 1 && client === 'claude' ? crypto.randomUUID() : null;
  const turns = [];
  for (const [turnIndex] of vector.turns.entries()) {
    let plan;
    try {
      plan = buildClientPlan(client, found.binary, target, paths, vector, protocol, { turnIndex, sessionId });
      secrets.push(...plan.secrets);
      writeConfigs(plan);
    } catch (error) {
      turns.push({
        turn_index: turnIndex,
        command: null,
        result: { exit_code: null, signal: null, timed_out: false, stdout: '', stderr: error.message },
      });
      break;
    }
    timeline.append('turn_started', { protocol, vector_id: vector.id, turn_index: turnIndex });
    let result;
    try {
      result = await executor(plan, executionOptions);
    } catch (error) {
      result = { exit_code: null, signal: null, timed_out: false, stdout: '', stderr: error.message };
    }
    turns.push({ turn_index: turnIndex, command: publicPlan(plan, surface), result });
    timeline.append('turn_finished', {
      protocol,
      vector_id: vector.id,
      turn_index: turnIndex,
      exit_code: result.exit_code,
      signal: result.signal,
      timed_out: result.timed_out,
    });
    if (result.exit_code !== 0 || result.timed_out || turnIndex === vector.turns.length - 1) break;
    sessionId = continuationId(client, result, sessionId);
    if (!sessionId) {
      turns.push({
        turn_index: turnIndex + 1,
        command: null,
        result: {
          exit_code: null,
          signal: null,
          timed_out: false,
          stdout: '',
          stderr: `${client} continuation session id was not observed`,
        },
      });
      break;
    }
  }
  return mergedTurnResult(turns);
}

async function runLocalClientAcceptance(options, dependencies = {}) {
  if (!options?.artifactRoot || !path.isAbsolute(options.artifactRoot)) {
    throw new Error('absolute artifactRoot is required');
  }
  if (typeof options.mockSnapshot !== 'function') throw new Error('mockSnapshot evidence reader is required');
  const registry = dependencies.registry || new OwnedResources(dependencies);
  const root = registry.addTempRoot(fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-local-clients-')));
  const versionProbe = dependencies.probeVersion || probeVersion;
  const snapshotRuns = dependencies.snapshotRuns || durableEvidence.snapshotRuns;
  const reconcileAttempt = dependencies.reconcileAttempt || durableEvidence.reconcileAttempt;
  const evaluateMockAttempt = dependencies.evaluateMockAttempt || durableEvidence.evaluateMockAttempt;
  const verifyIdle = dependencies.verifyIdle || durableEvidence.verifyIdle;
  const verifyGatewayExecutor = dependencies.gatewayExecutorEvidence || durableEvidence.gatewayExecutorEvidence;
  const waitForBarrierWaiting = dependencies.waitForBarrierWaiting || durableEvidence.waitForBarrierWaiting;
  const selectVectors = dependencies.vectorsFor || vectorsFor;
  let tmux = null;
  let surface = { status: 'skipped', surface: null, reason: 'discovery_not_completed' };
  const secrets = [];
  const clients = [];
  let cleanupErrors = [];
  let finalReconciliation = null;
  try {
    const discovery = (dependencies.discoverClients || discoverClients)(options.discovery);
    tmux = findExecutable(options.tmuxExecutable || 'tmux', options.discovery?.env?.PATH || process.env.PATH);
    surface = selectExecutionSurface(options.surface || 'auto', {
      tmux: Boolean(tmux),
      acpHeadless: Boolean(dependencies.acpHeadlessExecutor),
    });
    for (const client of ['claude', 'opencode', 'codex']) {
      const found = discovery[client];
      const timeline = createTimeline(dependencies.now);
      if (found.status !== 'ready') {
        clients.push({ name: client, status: 'skipped', reason: found.reason, discovery: found, timeline: [] });
        continue;
      }
      if (surface.status !== 'selected') {
        clients.push({ name: client, status: 'skipped', reason: surface.reason, discovery: found, timeline: [] });
        continue;
      }
      let version;
      try {
        version = await versionProbe(found.binary, { env: options.discovery?.env });
      } catch (error) {
        version = { status: 'failed', version: null, reason: error.message };
      }
      if (version.status !== 'ready') {
        clients.push({ name: client, status: 'fail', reason: version.reason, discovery: found, version, timeline: [] });
        continue;
      }
      const target = options.targets?.[client];
      if (!target) {
        clients.push({
          name: client,
          status: 'skipped',
          reason: 'target_not_configured',
          discovery: found,
          version,
          timeline: [],
        });
        continue;
      }
      if (!target.model || !target.apiKey || !target.gatewayBaseUrl) {
        clients.push({
          name: client,
          status: 'fail',
          reason: 'target_invalid',
          discovery: found,
          version,
          timeline: [],
        });
        continue;
      }
      const paths = clientPaths(root, client);
      const attempts = [];
      timeline.append('client_started', { client, surface: surface.surface });
      let failed = false;
      for (const protocol of CLIENT_PROTOCOLS[client]) {
        for (const vector of selectVectors(client, protocol)) {
          timeline.append('attempt_started', { protocol, vector_id: vector.id });
          const durableBefore = await snapshotRuns(target, dependencies.fetchImpl);
          const mockBefore = await options.mockSnapshot();
          const executor = dependencies.executePlan
            || (surface.surface === 'acp-headless' ? dependencies.acpHeadlessExecutor : executeTmux);
          const barrierAbort = new AbortController();
          const execution = executeVector({
            client,
            found,
            target,
            paths,
            vector,
            protocol,
            executor,
            executionOptions: {
              registry,
              timeoutMs: options.timeoutMs,
              surface: surface.surface,
              tmuxExecutable: tmux,
            },
            surface: surface.surface,
            secrets,
            timeline,
          }).finally(() => barrierAbort.abort());
          const barrierRelease = vector.kind === 'tools'
            ? waitForBarrierWaiting({
              before: mockBefore,
              mockSnapshot: options.mockSnapshot,
              signal: barrierAbort.signal,
              graceMs: options.timeoutMs ?? 180_000,
            }).then(() => options.releaseBarrier())
            : Promise.resolve();
          let result;
          const [executionOutcome, barrierOutcome] = await Promise.allSettled([execution, barrierRelease]);
          if (executionOutcome.status === 'fulfilled') result = executionOutcome.value;
          else {
            result = mergedTurnResult([{
              turn_index: 0,
              command: null,
              result: {
                exit_code: null,
                signal: null,
                timed_out: false,
                stdout: '',
                stderr: executionOutcome.reason?.message || String(executionOutcome.reason),
              },
            }]);
          }
          if (barrierOutcome.status === 'rejected') {
            const barrierMessage = barrierOutcome.reason?.message || String(barrierOutcome.reason);
            result.stderr = `${result.stderr}\n${barrierMessage}`.trim();
            result.exit_code = null;
          }
          const evaluation = evaluateAttempt(client, vector, result, protocol);
          let evidence = null;
          let evidenceError = null;
          try {
            const mockAfter = await options.mockSnapshot();
            evidence = {
              mock: evaluateMockAttempt(mockBefore, mockAfter, vector.expected),
              durable: await reconcileAttempt({
                target,
                before: durableBefore,
                expectedRuns: vector.expected.durable_runs,
                expectedStatuses: vector.expected.durable_statuses,
                fetchImpl: dependencies.fetchImpl,
              }),
            };
          } catch (error) {
            evidenceError = { name: error.name || 'Error', message: error.message || String(error) };
          }
          const passed = evaluation.pass && evidenceError === null;
          failed ||= !passed;
          for (const event of evaluation.observed_events) timeline.append(event, { protocol, vector_id: vector.id });
          timeline.append('attempt_finished', {
            protocol,
            vector_id: vector.id,
            exit_code: result.exit_code,
            signal: result.signal,
            timed_out: result.timed_out,
          });
          attempts.push({
            protocol,
            vector_id: vector.id,
            expected: publicExpectation(vector.expected),
            commands: result.turns.map((turn) => turn.command),
            result,
            evaluation,
            evidence,
            evidence_error: evidenceError,
          });
        }
      }
      timeline.append('client_exited', { client, status: failed ? 'fail' : 'pass' });
      clients.push({
        name: client,
        status: failed ? 'fail' : 'pass',
        reason: failed ? 'one_or_more_attempts_failed' : null,
        discovery: found,
        version,
        protocols: CLIENT_PROTOCOLS[client],
        attempts,
        timeline: timeline.snapshot(),
      });
    }
    finalReconciliation = {
      ...await verifyIdle(
        ['claude', 'opencode', 'codex'].map((client) => options.targets?.[client]).filter(Boolean),
        dependencies.fetchImpl,
      ),
      ...verifyGatewayExecutor(await options.mockSnapshot(), 0),
    };
  } catch (error) {
    clients.push({
      name: 'driver',
      status: 'fail',
      reason: 'driver_error',
      error: { name: error.name || 'Error', message: error.message || String(error) },
      timeline: [],
    });
  } finally {
    cleanupErrors = await registry.close();
  }
  const artifact = {
    schema_version: ARTIFACT_SCHEMA,
    run_id: crypto.randomUUID(),
    gate_role: 'mock_backed_local_client_acceptance',
    vector_manifest: {
      schema_version: VECTOR_MANIFEST.schema_version,
      vector_ids: VECTOR_MANIFEST.vectors.map((vector) => vector.id),
    },
    status: overallStatus(clients, cleanupErrors),
    surface_selection: surface,
    clients,
    final_reconciliation: finalReconciliation,
    cleanup: { status: cleanupErrors.length ? 'fail' : 'pass', errors: cleanupErrors },
  };
  const artifactPath = path.join(options.artifactRoot, 'local-client-acceptance.json');
  const safe = writeArtifact(artifactPath, artifact, secrets);
  return { ...safe, artifact_path: artifactPath };
}

module.exports = {
  assistantTextValues,
  clientPaths,
  continuationId,
  evaluateAttempt,
  includesMarker,
  overallStatus,
  publicPlan,
  runLocalClientAcceptance,
  structuredEvents,
  toolCalls,
  writeConfigs,
};
