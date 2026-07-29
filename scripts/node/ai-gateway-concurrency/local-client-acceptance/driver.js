'use strict';

const crypto = require('node:crypto');
const childProcess = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { writeArtifact, createTimeline } = require('./artifacts');
const {
  ARTIFACT_SCHEMA,
  CLIENT_PROTOCOLS,
  MEANINGFUL_GIT_VECTOR,
  TOOL_ASSETS,
  VECTOR_MANIFEST,
  buildClientPlan,
  selectExecutionSurface,
  vectorsFor,
} = require('./contract');
const { discoverClients, findExecutable, probeVersion } = require('./discovery');
const durableEvidence = require('./durable');
const { OwnedResources, executeTmux } = require('./lifecycle');
const { evaluateAttempt, structuredEvents } = require('./client-surface');

function clientPaths(root, client, gitRepo) {
  const clientRoot = path.join(root, client);
  const result = {
    root: clientRoot,
    config: path.join(clientRoot, 'config'),
    output: path.join(clientRoot, 'output'),
    toolAssets: {},
    gitRepo,
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

function gitWorkspaceFingerprint(repoPath) {
  if (typeof repoPath !== 'string' || !path.isAbsolute(repoPath)) {
    throw new Error('meaningful Git vector requires an absolute protected repository path');
  }
  const run = (...args) => {
    const completed = childProcess.spawnSync('git', args, {
      cwd: repoPath,
      encoding: 'utf8',
    });
    if (completed.status !== 0) {
      throw new Error(`protected git verification failed: ${completed.stderr || completed.stdout}`);
    }
    return completed.stdout;
  };
  const head = run('rev-parse', 'HEAD').trim();
  const status = run('status', '--porcelain=v1', '--untracked-files=all');
  const diff = run('diff', '--binary', 'HEAD', '--', '.');
  return {
    head,
    status_sha256: crypto.createHash('sha256').update(status).digest('hex'),
    diff_sha256: crypto.createHash('sha256').update(diff).digest('hex'),
  };
}

function verifyMeaningfulGitWorkspace(repoPath, before, expectedRevision) {
  const after = gitWorkspaceFingerprint(repoPath);
  if (expectedRevision && before.head !== expectedRevision) {
    throw new Error(`protected Git revision mismatch: expected ${expectedRevision}, observed ${before.head}`);
  }
  if (JSON.stringify(after) !== JSON.stringify(before)) {
    throw new Error('meaningful Git vector modified the protected repository');
  }
  return { repository: repoPath, ...after, unchanged: true };
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
    ...(expected.error_body ? {
      error_body: {
        match: 'decoded_contiguous_substring',
        sha256: crypto.createHash('sha256').update(expected.error_body).digest('hex'),
        utf8_bytes: Buffer.byteLength(expected.error_body),
      },
    } : {}),
  };
}

function overallStatus(clients, cleanupErrors) {
  if (cleanupErrors.length || clients.some((client) => client.status === 'fail')) return 'fail';
  if (clients.every((client) => client.status === 'skipped')) return 'skipped';
  if (clients.some((client) => client.status === 'skipped')) return 'partial';
  return 'pass';
}

function executionTargets(client, primary, matrix) {
  const candidates = [primary, ...(matrix?.[client] ?? [])].filter(Boolean);
  const seen = new Set();
  return candidates.flatMap((target) => {
    const identity = target.applicationId ?? `${target.provider}:${target.gatewayBaseUrl}`;
    if (seen.has(identity)) return [];
    seen.add(identity);
    return [{
      target,
      provider: target.provider ?? (target === primary ? client : 'unidentified'),
      fullVectors: target === primary || identity === primary?.applicationId,
    }];
  });
}

function crossTargetCanary(clients, required) {
  if (!required) return { status: 'not_requested', rows: [] };
  const rows = clients.flatMap((client) => {
    if (!CLIENT_PROTOCOLS[client.name]) return [];
    const attempts = (client.attempts ?? []).filter(
      (attempt) => attempt.vector_id === MEANINGFUL_GIT_VECTOR.id,
    );
    const providers = [...new Set(attempts.map((attempt) => attempt.provider_target))];
    return providers.map((provider) => {
      const providerAttempts = attempts.filter((attempt) => attempt.provider_target === provider);
      return {
        client: client.name,
        provider,
        protocols: providerAttempts.map((attempt) => attempt.protocol),
        status: providerAttempts.length === CLIENT_PROTOCOLS[client.name].length
          && providerAttempts.every((attempt) => attempt.status === 'pass')
          ? 'pass'
          : 'fail',
      };
    });
  });
  return {
    status: rows.length === 9 && rows.every((row) => row.status === 'pass') ? 'pass' : 'fail',
    rows,
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
  const verifyNetworkObserver = dependencies.networkObserverEvidence || durableEvidence.networkObserverEvidence;
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
      const primaryTarget = options.targets?.[client];
      if (!primaryTarget) {
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
      if (!primaryTarget.model || !primaryTarget.apiKey || !primaryTarget.gatewayBaseUrl) {
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
      const attempts = [];
      timeline.append('client_started', { client, surface: surface.surface });
      let failed = false;
      for (const targetRow of executionTargets(client, primaryTarget, options.targetMatrix)) {
        const { target, provider, fullVectors } = targetRow;
        if (!target.model || !target.apiKey || !target.gatewayBaseUrl) {
          failed = true;
          attempts.push({ provider_target: provider, status: 'fail', reason: 'target_invalid' });
          continue;
        }
        const paths = clientPaths(root, `${client}-${provider}`, options.gitRepoPath ?? null);
        for (const protocol of CLIENT_PROTOCOLS[client]) {
          const vectors = fullVectors
            ? selectVectors(client, protocol)
            : [MEANINGFUL_GIT_VECTOR];
          for (const vector of vectors) {
            timeline.append('attempt_started', { provider, protocol, vector_id: vector.id });
            const gitWorkspaceBefore = vector.id === MEANINGFUL_GIT_VECTOR.id
              ? gitWorkspaceFingerprint(paths.gitRepo)
              : null;
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
            }
            const evaluation = evaluateAttempt(client, vector, result, protocol);
            let evidence = null;
            let evidenceError = null;
            try {
              const mockAfter = await options.mockSnapshot();
              const mock = evaluateMockAttempt(mockBefore, mockAfter, vector.expected);
              const expectedDurableRuns = vector.expected.durable_runs === 'provider_requests'
                ? mock.arrivals
                : vector.expected.durable_runs;
              evidence = {
                mock,
                durable: await reconcileAttempt({
                  target,
                  before: durableBefore,
                  expectedRuns: expectedDurableRuns,
                  expectedStatuses: vector.expected.durable_statuses,
                  expectedErrorBody: vector.expected.error_body ?? null,
                  fetchImpl: dependencies.fetchImpl,
                }),
                ...(vector.id === MEANINGFUL_GIT_VECTOR.id
                  ? {
                    git_workspace: verifyMeaningfulGitWorkspace(
                      paths.gitRepo,
                      gitWorkspaceBefore,
                      options.gitRepoRevision ?? null,
                    ),
                  }
                  : {}),
              };
            } catch (error) {
              evidenceError = { name: error.name || 'Error', message: error.message || String(error) };
            }
            const passed = evaluation.pass && evidenceError === null;
            failed ||= !passed;
            for (const event of evaluation.observed_events) {
              timeline.append(event, { provider, protocol, vector_id: vector.id });
            }
            timeline.append('attempt_finished', {
              provider,
              protocol,
              vector_id: vector.id,
              exit_code: result.exit_code,
              signal: result.signal,
              timed_out: result.timed_out,
            });
            attempts.push({
              provider_target: provider,
              target_application_id: target.applicationId ?? null,
              protocol,
              vector_id: vector.id,
              status: passed ? 'pass' : 'fail',
              expected: publicExpectation(vector.expected),
              commands: result.turns.map((turn) => turn.command),
              result,
              evaluation,
              evidence,
              evidence_error: evidenceError,
            });
          }
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
        provider_targets: executionTargets(client, primaryTarget, options.targetMatrix)
          .map((row) => row.provider),
        attempts,
        timeline: timeline.snapshot(),
      });
    }
    const finalMockSnapshot = await options.mockSnapshot();
    finalReconciliation = {
      ...await verifyIdle(
        [...new Map(
          ['claude', 'opencode', 'codex'].flatMap((client) => (
            executionTargets(client, options.targets?.[client], options.targetMatrix)
              .map((row) => [row.target.applicationId ?? row.provider, row.target])
          )),
        ).values()],
        dependencies.fetchImpl,
      ),
      ...verifyGatewayExecutor(finalMockSnapshot, 0),
      ...verifyNetworkObserver(finalMockSnapshot, 0),
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
  const matrixEvidence = crossTargetCanary(clients, options.requireCrossTargetMatrix === true);
  if (matrixEvidence.status === 'fail') {
    clients.push({
      name: 'cross-target-canary',
      status: 'fail',
      reason: 'three_client_three_provider_matrix_incomplete',
      timeline: [],
    });
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
    cross_target_canary: matrixEvidence,
    final_reconciliation: finalReconciliation,
    cleanup: { status: cleanupErrors.length ? 'fail' : 'pass', errors: cleanupErrors },
  };
  const artifactPath = path.join(options.artifactRoot, 'local-client-acceptance.json');
  const safe = writeArtifact(artifactPath, artifact, secrets);
  return { ...safe, artifact_path: artifactPath };
}

module.exports = {
  clientPaths,
  continuationId,
  evaluateAttempt,
  gitWorkspaceFingerprint,
  overallStatus,
  publicPlan,
  crossTargetCanary,
  executionTargets,
  runLocalClientAcceptance,
  structuredEvents,
  verifyMeaningfulGitWorkspace,
  writeConfigs,
};
