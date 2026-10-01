#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { StringDecoder } = require('node:string_decoder');
const { spawn, execFileSync } = require('node:child_process');
const { Rpc } = require('../worker-lifecycle-acceptance/rpc.cjs');
const { evaluate, validateSubagent, isUsefulTool, MIN_SPAN_MS } = require('./oracle.cjs');
const { clientConfig, clientLogFilter, retainStderrLine } = require('./client-config.cjs');
const { collectMailboxEvidence, clientTraces } = require('./mailbox-evidence.cjs');
const { validateContextProbe } = require('./context-error.cjs');
const { collectInstructionOrder } = require('./instruction-order.cjs');

const WORKSPACE = path.resolve(process.env.RLS_WORKSPACE || '/home/taichuy/git/1flowbase_latest');
const BASE_URL = process.env.RLS_BASE_URL || 'http://127.0.0.1:7600/v1';
const APP_ID = process.env.RLS_APP_ID || '01a08ebe-dcba-7ec1-a448-f466d9aa23f8';
const KEY_FILE = process.env.RLS_KEY_FILE || '/home/taichuy/git/1flowbase/tmp/test-tmp-kay.md';
const SUBAGENT_MODEL = process.env.RLS_SUBAGENT_MODEL || 'gpt-6-sol';
let artifactDir;
const required = (name) => { if (!process.env[name]) throw new Error(`Missing ${name}`); return process.env[name]; };
const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');
const fileDigest = (file) => sha256(fs.readFileSync(file));
const input = (value) => [{ type: 'text', text: value, text_elements: [] }];

function processIdentity(pid) {
  if (!Number.isSafeInteger(pid) || pid <= 1) throw new Error('Invalid process PID');
  const base = `/proc/${pid}`;
  const stat = fs.readFileSync(`${base}/stat`, 'utf8').split(') ').pop().split(' ');
  return { pid, ppid: Number(stat[1]), startTime: stat[19], exe: fs.readlinkSync(`${base}/exe`),
    digest: fileDigest(`${base}/exe`),
    argv: fs.readFileSync(`${base}/cmdline`, 'utf8').split('\0').filter(Boolean) };
}

function identities() {
  const baseline = required('RLS_BASELINE_SHA');
  const patchDigest = required('RLS_PATCH_SHA256');
  const patchFile = path.resolve(required('RLS_PATCH_FILE'));
  const candidateRoot = path.resolve(required('RLS_CANDIDATE_ROOT'));
  const candidateGit = (...args) => execFileSync('git', ['-C', candidateRoot, ...args],
    { encoding: 'utf8', maxBuffer: 4096 }).trim();
  const apiDigest = required('RLS_API_SHA256');
  const pluginDigest = required('RLS_PLUGIN_SHA256');
  const api = processIdentity(Number(required('RLS_API_PID')));
  const plugin = processIdentity(Number(required('RLS_PLUGIN_PID')));
  const apiExe = fs.realpathSync(required('RLS_API_EXE'));
  const pluginExe = fs.realpathSync(required('RLS_PLUGIN_EXE'));
  if (!/^[a-f0-9]{40}$/.test(baseline) || !/^[a-f0-9]{64}$/.test(patchDigest)
      || !/^[a-f0-9]{64}$/.test(apiDigest) || !/^[a-f0-9]{64}$/.test(pluginDigest))
    throw new Error('Invalid candidate provenance digest format');
  if (candidateGit('rev-parse', 'HEAD') !== baseline) throw new Error('Candidate assembly baseline changed');
  if (fileDigest(patchFile) !== patchDigest) throw new Error('Candidate patch digest mismatch');
  const currentPatch = execFileSync('git', ['-C', candidateRoot, 'diff', '--binary', 'HEAD'],
    { maxBuffer: 8 * 1024 * 1024 });
  if (sha256(currentPatch) !== patchDigest) throw new Error('Candidate source no longer matches frozen patch');
  if (api.exe !== apiExe || api.digest !== apiDigest || !api.argv.includes(required('RLS_API_ARG')))
    throw new Error('Live API binary/argv differs from requested candidate');
  if (plugin.exe !== pluginExe || plugin.digest !== pluginDigest || plugin.ppid !== api.pid
      || !plugin.argv.includes(required('RLS_PLUGIN_ARG')))
    throw new Error('Live plugin identity differs from requested candidate');
  const publicIdentity = (p) => ({ pid: p.pid, ppid: p.ppid, startTime: p.startTime,
    exe: p.exe, digest: p.digest });
  return { baseline, patchDigest, patchFile, candidateRoot, api: publicIdentity(api),
    plugin: publicIdentity(plugin), apiDigest, pluginDigest };
}

function credential() {
  const content = fs.readFileSync(KEY_FILE, 'utf8').trim();
  const matches = [...new Set(content.match(/\bsk-[A-Za-z0-9_-]+/g) || [])];
  const value = matches.length === 1 ? matches[0] : (!content.includes('\n') && !matches.length ? content : null);
  if (!value) throw new Error('Key file does not contain one unambiguous credential');
  return value;
}

function auditInventory() {
  const files = execFileSync('rg', ['--files', 'api/crates', 'api/apps', 'web', 'scripts/node'],
    { cwd: WORKSPACE, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 }).trim().split('\n')
    .filter((name) => /\.(rs|tsx?|jsx?|cjs|mjs)$/.test(name));
  if (files.length < 20) throw new Error('Insufficient real source audit inventory');
  return files;
}

async function main() {
  const runId = `${new Date().toISOString().replace(/[:.]/g, '-')}-${crypto.randomUUID()}`;
  const out = path.join(process.env.RLS_ARTIFACT_ROOT || path.join(WORKSPACE, 'tmp/test-governance/2175'), runId);
  artifactDir = out;
  fs.mkdirSync(out, { recursive: true, mode: 0o700 });
  const candidate = identities();
  if (required('RLS_APP_ID') !== APP_ID) throw new Error('Requested app ID differs from target application');
  const catalogPath = path.resolve(required('RLS_MODEL_CATALOG'));
  const catalogText = fs.readFileSync(catalogPath, 'utf8');
  if (sha256(catalogText) !== required('RLS_MODEL_CATALOG_SHA256'))
    throw new Error('Model catalog digest differs from requested catalog');
  const catalog = JSON.parse(catalogText);
  if (!['gpt-6-luna', SUBAGENT_MODEL].every((id) => catalog.models?.some((m) => m.slug === id)))
    throw new Error('Model catalog lacks requested luna/sol slugs');
  const secret = credential();
  const codex = process.env.RLS_CODEX || 'codex';
  const codexPath = codex.includes('/') ? fs.realpathSync(codex)
    : fs.realpathSync(execFileSync('which', [codex], { encoding: 'utf8', maxBuffer: 4096 }).trim());
  const codexVersion = execFileSync(codex, ['--version'], { encoding: 'utf8', maxBuffer: 4096 }).trim();
  if (codexVersion !== required('RLS_CODEX_VERSION')) throw new Error('Installed Codex version differs from requested client');
  const scopes = auditInventory();
  const home = path.join(out, 'codex-home');
  fs.mkdirSync(home, { mode: 0o700 });
  const catalogCopy = path.join(home, 'model-catalog.json');
  fs.writeFileSync(catalogCopy, catalogText, { mode: 0o600 });
  const redact = (value) => JSON.stringify(value).split(secret).join('[REDACTED]')
    .replace(/Bearer\s+[^\s"\\]+/gi, 'Bearer [REDACTED]')
    .replace(/sk-[A-Za-z0-9._-]{8,}/g, '[REDACTED]');
  const startedUtc = new Date().toISOString();
  const started = process.hrtime.bigint();
  const at = () => Number(process.hrtime.bigint() - started) / 1e6;
  const events = [];
  const record = (value) => {
    if (value.method === 'turn/start' && value.params?.input?.[0]?.text === contextInput) {
      value = { ...value, params: { ...value.params, input: [{ type: 'text', text: '[context probe input recorded by digest]' }] },
        contextProbeInput: { bytes: Buffer.byteLength(contextInput), sha256: sha256(contextInput) } };
    }
    const safe = JSON.parse(redact({ atMs: at(), ...value }));
    events.push(safe);
    fs.appendFileSync(path.join(out, 'timeline.jsonl'), `${JSON.stringify(safe)}\n`, { mode: 0o600 });
  };
  const save = (name, data) => fs.writeFileSync(path.join(out, name), `${redact(data)}\n`, { mode: 0o600 });
  const sentinel = `RLS_SENTINEL_${crypto.randomUUID()}`;
  const steerNonce = `RLS_STEER_${crypto.randomUUID()}`;
  const steerNonces = [steerNonce, `RLS_STEER_${crypto.randomUUID()}`];
  const sentinelFile = path.join(out, 'sentinel.txt');
  fs.writeFileSync(sentinelFile, sentinel, { mode: 0o600 });
  const env = { PATH: process.env.PATH, HOME: home, CODEX_HOME: home, RLS_GATEWAY_KEY: secret,
    RUST_LOG: clientLogFilter,
    LANG: 'C.UTF-8', NO_PROXY: '127.0.0.1,localhost,::1', no_proxy: '127.0.0.1,localhost,::1' };
  fs.writeFileSync(path.join(home, 'config.toml'),
    clientConfig({ catalogPath: catalogCopy, baseUrl: BASE_URL }), { mode: 0o600 });
  const minSpanMs = Number(process.env.RLS_MIN_SPAN_MS || MIN_SPAN_MS);
  if (!Number.isSafeInteger(minSpanMs) || minSpanMs < 1_800_000)
    throw new Error('RLS_MIN_SPAN_MS must require at least 30 minutes of useful source work');
  const contextInput = process.env.RLS_CONTEXT_INPUT_FILE
    ? fs.readFileSync(path.resolve(process.env.RLS_CONTEXT_INPUT_FILE), 'utf8') : null;
  if (contextInput !== null && !contextInput.length) throw new Error('Context probe input is empty');
  const meta = { candidate, appId: APP_ID, baseUrl: BASE_URL, codexVersion,
    codexDigest: fileDigest(codexPath),
    modelCatalogPath: catalogPath, modelCatalogDigest: sha256(catalogText), workspace: WORKSPACE,
    model: 'gpt-6-luna', reasoningEffort: 'max', subagentModel: SUBAGENT_MODEL, subagentEffort: 'medium',
    minSpanMs, contextProbeRequired: contextInput !== null,
    contextProbeInput: contextInput === null ? null : { bytes: Buffer.byteLength(contextInput), sha256: sha256(contextInput) },
    requestedTransport: 'responses_websocket', retryPolicy: 'Codex provider defaults; no retry overrides',
    sentinel, steerNonce, steerNonces, orderedSteerRequired: process.env.RLS_ORDERED_STEER === 'true', startedUtc,
    fixtureDigests: Object.fromEntries(['run.cjs', 'oracle.cjs', 'client-config.cjs', 'mailbox-evidence.cjs', 'context-error.cjs', 'instruction-order.cjs', '../worker-lifecycle-acceptance/rpc.cjs']
      .map((name) => [name, fileDigest(path.resolve(__dirname, name))])) };
  const deadlineMs = Number(process.env.RLS_DEADLINE_MS || 7_200_000);
  if (!Number.isSafeInteger(deadlineMs) || deadlineMs < minSpanMs + 120_000)
    throw new Error('RLS_DEADLINE_MS must be a finite integer with time for the required work span');
  save('manifest.json', meta);
  const child = spawn(codex, ['app-server', '--stdio'], { cwd: WORKSPACE, env, stdio: ['pipe', 'pipe', 'pipe'] });
  const stderrDecoder = new StringDecoder('utf8');
  let stderrCarry = '';
  const retainLines = (text, final = false) => {
    const lines = (stderrCarry + text).split('\n');
    stderrCarry = lines.pop();
    if (final && stderrCarry) { lines.push(stderrCarry); stderrCarry = ''; }
    for (const line of lines) {
      if (retainStderrLine(line)) record({ kind: 'client/stderr', text: `${line}\n` });
    }
  };
  child.stderr.on('data', (data) => retainLines(stderrDecoder.write(data)));
  child.stderr.on('end', () => retainLines(stderrDecoder.end(), true));
  const rpc = new Rpc(child, record);
  const deadline = at() + deadlineMs;
  const remaining = () => {
    const ms = deadline - at();
    if (ms <= 0) throw new Error('Finite run deadline exceeded');
    return Math.min(ms, 900_000);
  };
  const terminal = (id) => rpc.wait((e) => e.method === 'turn/completed'
    && e.params?.threadId === meta.threadId && e.params.turn?.id === id, remaining());
  const startTurn = async (prompt) => {
    const result = await rpc.request('turn/start', { threadId: meta.threadId, input: input(prompt),
      model: 'gpt-6-luna', effort: 'max' }, remaining());
    const id = result.turn.id;
    record({ kind: 'turn/invoked', threadId: meta.threadId, turnId: id });
    return id;
  };
  const workPrompt = (index, extra = '') => {
    const dimensions = ['ownership and cancellation', 'state transitions', 'error propagation',
      'algorithmic complexity', 'field contracts', 'tests and counterexamples'];
    const file = scopes[index % scopes.length];
    const dimension = dimensions[Math.floor(index / scopes.length) % dimensions.length];
    const quotedFile = `'${file.replace(/'/g, "'\\''")}'`;
    const sourceCommand = `sed -n '1,180p;181q' -- ${quotedFile} | nl -ba | head -c 12000`;
    return `Continue the SAME root thread read-only source audit. Work unit ${index}: inspect the actual workspace file ${quotedFile} and its direct callers for ${dimension}. ${extra} Execute this read-only source command with a tool: ${sourceCommand}. The first turn's sentinel read and child-spawn instructions take priority. From the fresh source, trace a concrete path through its direct callers and cite file/line evidence. This audit supplies real traffic for gateway acceptance; prose quality and remembered values are not being graded. No sleep, idle waits, builds, network/auth calls, file edits, unbounded stdout or invented observations.`;
  };
  let failure;
  try {
    await rpc.request('initialize', { clientInfo: { name: 'responses_long_session_2175', version: '1' },
      capabilities: { experimentalApi: true } }, remaining());
    rpc.send({ method: 'initialized', params: {} });
    const models = await rpc.request('model/list', {}, remaining());
    for (const id of ['gpt-6-luna', SUBAGENT_MODEL]) {
      if (!models.data?.some((model) => model.id === id)) throw new Error(`Client model/list missing ${id}`);
    }
    const thread = await rpc.request('thread/start', { model: 'gpt-6-luna', modelProvider: 'candidate',
      cwd: WORKSPACE, approvalPolicy: 'never', sandbox: 'read-only', ephemeral: false,
      developerInstructions: `Authorized real read-only source audit. Use ${SUBAGENT_MODEL} medium for one real bounded subagent and consume its completion notification before finishing. Keep doing substantive source work while it runs; do not poll or call wait_agent during that work. No nested subagents. Cap every tool output. Do not edit files.` }, remaining());
    meta.threadId = thread.thread.id;
    save('manifest.json', meta);
    const subagentFile = scopes[1];
    const first = await startTurn(workPrompt(0, `First cat ${sentinelFile} through a tool. Spawn exactly one real ${SUBAGENT_MODEL} reasoning medium subagent with this bounded read-only task: inspect ${subagentFile} and only its direct callers, cite concrete lines, and return one state/error-path finding plus one counterexample. While it runs, continue your own substantive inspection of ${scopes[0]} and its direct callers, tracing ownership, cancellation and terminal propagation. Do not poll or call wait_agent: let its completion notify your parent mailbox during your ongoing source work, then consume its result before ending the turn. Do not delegate general exploration or spawn another agent.`));
    meta.mailboxTurnId = first;
    meta.subagentAuditFile = subagentFile;
    save('manifest.json', meta);
    if ((await terminal(first)).params.turn.status !== 'completed') throw new Error('Initial audit turn failed');
    const firstTools = events.filter((e) => e.method === 'item/completed' && e.params?.turnId === first
      && e.params.item?.type === 'commandExecution' && e.params.item.exitCode === 0);
    if (!firstTools.some((e) => String(e.params.item.aggregatedOutput || '').includes(sentinel)))
      throw new Error('Sentinel was not read by a successful tool');
    const activity = events.find((e) => e.method === 'item/completed'
      && e.params?.threadId === meta.threadId && e.params.turnId === first
      && e.params.item?.type === 'subAgentActivity' && e.params.item.kind === 'started');
    const childId = activity?.params.item.agentThreadId;
    if (!childId) throw new Error('Initial turn lacks an actual subagent start');
    const sessions = path.join(home, 'sessions');
    const rollouts = fs.readdirSync(sessions, { recursive: true })
      .filter((name) => name.endsWith(`-${childId}.jsonl`));
    if (rollouts.length !== 1) throw new Error('Cannot uniquely locate the actual child rollout');
    meta.subagentRolloutPath = path.join(sessions, rollouts[0]);
    const subagent = validateSubagent(events, meta);
    if (subagent.errors.length) throw new Error(subagent.errors.join('; '));
    record({ kind: 'scenario/subagent-verified', childThreadId: childId });
    const captured = clientTraces(events, meta);
    if (!['sample_started', 'sample_closed', 'post_sampling', 'websocket_connected']
      .every(kind => captured.some(trace => trace.kind === kind)))
      throw new Error('First turn lacks required source-backed client sampling trace signals');
    record({ kind: 'scenario/client-traces-verified' });
    save('manifest.json', meta);
    const compactSince = rpc.history.length;
    await rpc.request('thread/compact/start', { threadId: meta.threadId }, remaining());
    const compact = await rpc.wait((e) => e.method === 'item/completed'
      && e.params?.threadId === meta.threadId && e.params.item?.type === 'contextCompaction', remaining(), compactSince);
    await terminal(compact.params.turnId);
    const semanticSince = rpc.history.length;
    const second = await startTurn(workPrompt(1, 'Continue the same source audit after the completed context compaction.'));
    meta.continuityTurnId = second;
    meta.steerTurnId = second;
    await rpc.wait((e) => e.params?.threadId === meta.threadId && e.params?.turnId === second
      && ['item/agentMessage/delta', 'item/reasoning/summaryTextDelta', 'item/reasoning/textDelta'].includes(e.method)
      && typeof e.params.delta === 'string' && e.params.delta.trim(), remaining(), semanticSince);
    await rpc.request('turn/steer', { threadId: meta.threadId, expectedTurnId: second,
      input: input(`Active-turn instruction ${steerNonce}: continue the same tool-backed source audit.`) }, remaining());
    record({ kind: 'steer/accepted', turnId: second, ordinal: 1 });
    if (meta.orderedSteerRequired) {
      await rpc.request('turn/steer', { threadId: meta.threadId, expectedTurnId: second,
        input: input(`Second active-turn instruction ${steerNonces[1]}: after the prior instruction, inspect one additional direct caller with a tool and finish the source audit.`) }, remaining());
      record({ kind: 'steer/accepted', turnId: second, ordinal: 2 });
    }
    if ((await terminal(second)).params.turn.status !== 'completed') throw new Error('Steered continuity turn failed');
    const continuity = evaluate(events, { ...meta, durationMs: at() });
    const prerequisiteError = continuity.errors.find(error => [
      'missing successful same-thread post-compaction continuity', 'active-turn steer lifecycle order invalid',
    ].includes(error));
    if (prerequisiteError) throw new Error(`Early continuity prerequisite failed: ${prerequisiteError}`);
    record({ kind: 'scenario/continuity-verified' });
    const firstUsefulTool = events.find((e) => e.params?.threadId === meta.threadId && isUsefulTool(e));
    if (!firstUsefulTool) throw new Error('Initial scenarios lack a real source inspection');
    const requiredLastToolAt = firstUsefulTool.atMs + minSpanMs;
    let index = 2;
    while (at() < requiredLastToolAt) {
      if (index >= scopes.length * 6) throw new Error('Real audit inventory exhausted before the required work span');
      const id = await startTurn(workPrompt(index++));
      if ((await terminal(id)).params.turn.status !== 'completed') throw new Error(`Audit turn ${id} failed`);
      save('progress.json', { threadId: meta.threadId, atMs: at(), workUnits: index, lastTurnId: id });
    }
    // Ensure the final successful tool, not just wall time, is after the hour mark.
    const last = await startTurn(workPrompt(index++, 'Final source inspection and evidence summary for this same thread.'));
    if ((await terminal(last)).params.turn.status !== 'completed') throw new Error('Final audit turn failed');
    if (contextInput !== null) {
      meta.contextProbeAfterTurnId = last;
      meta.contextProbeTurnId = await startTurn(contextInput);
      save('manifest.json', meta);
      await terminal(meta.contextProbeTurnId);
      const context = validateContextProbe(events, meta);
      if (context.errors.length) throw new Error(context.errors.join('; '));
      record({ kind: 'scenario/context-window-exceeded-verified', turnId: meta.contextProbeTurnId });
    }
  } catch (error) {
    failure = error.message;
    record({ kind: 'fixture/failure', error: failure });
  } finally {
    meta.durationMs = at();
    meta.finishedUtc = new Date().toISOString();
    meta.failure = failure || null;
    meta.mailboxEvidence = await collectMailboxEvidence({ events, meta, out,
      repositoryRoot: process.env.RLS_DATABASE_REPOSITORY || '/home/taichuy/git/1flowbase_gateway' });
    meta.instructionOrderEvidence = collectInstructionOrder({ events, meta, out,
      repositoryRoot: process.env.RLS_DATABASE_REPOSITORY || '/home/taichuy/git/1flowbase_gateway' });
    const verdict = evaluate(events, meta);
    save('manifest.json', meta);
    save('verdict.json', verdict);
    child.stdin.end();
    child.kill('SIGTERM');
    const force = setTimeout(() => { if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL'); }, 5000);
    await new Promise((resolve) => {
      if (child.exitCode !== null || child.signalCode !== null) resolve();
      else child.once('exit', resolve);
    });
    clearTimeout(force);
    console.log(JSON.stringify({ status: verdict.status, errors: verdict.errors,
      unverified: verdict.unverified, usefulWorkSpanMs: verdict.usefulWorkSpanMs, artifacts: out }));
    process.exitCode = verdict.pass ? 0 : 1;
  }
}

main().catch((error) => {
  const message = `Fixture setup failed: ${error.message}`;
  if (artifactDir) fs.writeFileSync(path.join(artifactDir, 'setup-error.log'), `${message}\n`, { mode: 0o600 });
  console.error(`${message} (artifacts: ${artifactDir || 'unavailable'})`);
  process.exitCode = 1;
});
