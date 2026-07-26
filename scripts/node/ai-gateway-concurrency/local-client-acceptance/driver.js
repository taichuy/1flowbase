'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { writeArtifact, createTimeline } = require('./artifacts');
const {
  ARTIFACT_SCHEMA, CLIENT_PROTOCOLS, TEXT_SENTINEL, TEXT_VECTOR,
  TOOL_RESULT_SENTINEL, TOOL_VECTOR, buildClientPlan, selectExecutionSurface,
} = require('./contract');
const { discoverClients, findExecutable, probeVersion } = require('./discovery');
const durableEvidence = require('./durable');
const { OwnedResources, executeTmux } = require('./lifecycle');

function clientPaths(root, client) {
  const clientRoot = path.join(root, client);
  const result = {
    root: clientRoot,
    config: path.join(clientRoot, 'config'),
    output: path.join(clientRoot, 'output'),
    toolFile: path.join(clientRoot, 'output', 'tool-vector.txt'),
  };
  fs.mkdirSync(result.config, { recursive: true, mode: 0o700 });
  fs.mkdirSync(result.output, { recursive: true, mode: 0o700 });
  fs.writeFileSync(result.toolFile, `${TOOL_RESULT_SENTINEL}\n`, { mode: 0o600 });
  return result;
}

function writeConfigs(plan) {
  for (const config of plan.configFiles) {
    fs.mkdirSync(path.dirname(config.path), { recursive: true, mode: 0o700 });
    fs.writeFileSync(config.path, config.content, { mode: 0o600 });
  }
}

function publicPlan(plan, surface) {
  return {
    surface,
    executable: plan.invocation.executable,
    cwd: plan.invocation.cwd,
    args: plan.invocation.args,
    environment: Object.fromEntries(Object.keys(plan.environment).sort().map((name) => [name, plan.environment[name]])),
    generated_config_paths: plan.configFiles.map((config) => config.path),
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

function isAssistantText(client, event) {
  if (client === 'codex') {
    return event.type === 'item.completed' && event.item?.type === 'agent_message';
  }
  if (client === 'claude') return event.type === 'assistant' || event.type === 'result';
  return event.type === 'message.part.updated'
    && event.properties?.part?.type === 'text';
}

function isToolCall(client, event) {
  if (client === 'codex') {
    return ['command_execution', 'mcp_tool_call', 'tool_call'].includes(event.item?.type);
  }
  if (client === 'claude') {
    return event.type === 'assistant'
      && event.message?.content?.some?.((part) => part.type === 'tool_use');
  }
  return event.type === 'message.part.updated'
    && ['tool', 'tool_call'].includes(event.properties?.part?.type);
}

function isToolResult(client, event) {
  if (!includesMarker(event, TOOL_RESULT_SENTINEL)) return false;
  if (client === 'codex') return ['command_execution', 'mcp_tool_call', 'tool_call'].includes(event.item?.type);
  if (client === 'claude') {
    return event.type === 'user' || event.message?.content?.some?.((part) => part.type === 'tool_result');
  }
  return event.type === 'message.part.updated'
    && ['tool', 'tool_result'].includes(event.properties?.part?.type);
}

function evaluateAttempt(client, vector, result, protocol = null) {
  if (result.exit_code !== 0 || result.timed_out) {
    return { pass: false, reason: 'client_process_failed', observed_events: [] };
  }
  if (client === 'codex' && protocol === 'responses_websocket') {
    const diagnostics = `${result.stdout || ''}\n${result.stderr || ''}`;
    if (/falling back to HTTP|fallback_to_http/iu.test(diagnostics)) {
      return { pass: false, reason: 'responses_websocket_http_fallback', observed_events: [] };
    }
    if (!/model_client\.stream_responses_websocket|transport\s*=\s*["']?responses_websocket/iu.test(diagnostics)) {
      return { pass: false, reason: 'responses_websocket_evidence_missing', observed_events: [] };
    }
  }
  const events = structuredEvents(result.stdout || '');
  if (vector.kind === 'text') {
    const observed = events.some((event) => isAssistantText(client, event)
      && includesMarker(event, TEXT_SENTINEL));
    return {
      pass: observed,
      reason: observed ? null : 'canonical_text_sentinel_missing',
      observed_events: observed ? ['canonical_text_sentinel_observed'] : [],
    };
  }
  const toolCallIndex = events.findIndex((event) => isToolCall(client, event));
  const toolResultIndex = events.findIndex((event, index) => index > toolCallIndex && isToolResult(client, event));
  const finalIndex = events.findIndex((event, index) => index > toolResultIndex
    && isAssistantText(client, event)
    && includesMarker(event, TOOL_VECTOR.expected.final_marker));
  const toolCall = toolCallIndex >= 0;
  const toolResult = toolResultIndex > toolCallIndex;
  const finalMarker = finalIndex > toolResultIndex;
  const observed = [];
  if (toolCall) observed.push('tool_call_observed');
  if (toolResult) observed.push('tool_result_observed', 'second_turn_observed');
  if (finalMarker) observed.push('final_marker_observed');
  const pass = toolCall && toolResult && finalMarker;
  return { pass, reason: pass ? null : 'tool_two_turn_evidence_missing', observed_events: observed };
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
        clients.push({ name: client, status: 'skipped', reason: 'target_not_configured', discovery: found, version, timeline: [] });
        continue;
      }
      if (!target.model || !target.apiKey || !target.gatewayBaseUrl) {
        clients.push({ name: client, status: 'fail', reason: 'target_invalid', discovery: found, version, timeline: [] });
        continue;
      }
      const paths = clientPaths(root, client);
      const attempts = [];
      timeline.append('client_started', { client, surface: surface.surface });
      let failed = false;
      for (const protocol of CLIENT_PROTOCOLS[client]) {
        for (const vector of [TEXT_VECTOR, TOOL_VECTOR]) {
          const plan = buildClientPlan(client, found.binary, target, paths, vector, protocol);
          secrets.push(...plan.secrets);
          writeConfigs(plan);
          timeline.append('attempt_started', { protocol, vector_id: vector.id });
          const durableBefore = await snapshotRuns(target, dependencies.fetchImpl);
          const mockBefore = await options.mockSnapshot();
          const executor = dependencies.executePlan
            || (surface.surface === 'acp-headless' ? dependencies.acpHeadlessExecutor : executeTmux);
          let result;
          try {
            result = await executor(plan, {
              registry,
              timeoutMs: options.timeoutMs,
              surface: surface.surface,
              tmuxExecutable: tmux,
              onFirstMarker: vector.kind === 'tool' ? options.releaseBarrier : undefined,
            });
          } catch (error) {
            result = {
              exit_code: null,
              signal: null,
              timed_out: false,
              stdout: '',
              stderr: error.message,
            };
          }
          const evaluation = evaluateAttempt(client, vector, result, protocol);
          let evidence = null;
          let evidenceError = null;
          try {
            const expectedRuns = vector.expected.durable_runs;
            const mockAfter = await options.mockSnapshot();
            evidence = {
              mock: evaluateMockAttempt(mockBefore, mockAfter, expectedRuns),
              durable: await reconcileAttempt({
                target,
                before: durableBefore,
                expectedRuns,
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
            protocol, vector_id: vector.id, exit_code: result.exit_code,
            signal: result.signal, timed_out: result.timed_out,
          });
          attempts.push({
            protocol,
            vector_id: vector.id,
            expected: vector.expected,
            command: publicPlan(plan, surface.surface),
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
    finalReconciliation = await verifyIdle(
      ['claude', 'opencode', 'codex'].map((client) => options.targets?.[client]).filter(Boolean),
      dependencies.fetchImpl,
    );
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
  clientPaths,
  evaluateAttempt,
  includesMarker,
  overallStatus,
  publicPlan,
  runLocalClientAcceptance,
  writeConfigs,
};
