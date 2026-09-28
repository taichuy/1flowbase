#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawn, execFileSync } = require('node:child_process');
const { Rpc } = require('./rpc.cjs');
const { snapshot, discover, killVerified } = require('./proc.cjs');
const { evaluate } = require('./oracle.cjs');
const required = (name) => { if (!process.env[name]) throw new Error(`Required environment input: ${name}`); return process.env[name]; };
const textInput = (text) => [{ type: 'text', text, text_elements: [] }];
async function main() {
  const workspace = '/home/taichuy/git/1flowbase_latest';
  const out = path.resolve(process.env.WL_ARTIFACT_DIR || path.join(workspace, 'tmp/test-governance/2153/long-task'));
  if (!out.endsWith('/tmp/test-governance/2153/long-task')) throw new Error('Artifact directory must end in tmp/test-governance/2153/long-task');
  if (fs.existsSync(path.join(out, 'timeline.jsonl'))) throw new Error('Existing timeline: archive the prior run before starting a new one');
  const baseUrl = process.env.WL_BASE_URL || 'http://127.0.0.1:7600/v1';
  if (baseUrl !== 'http://127.0.0.1:7600/v1') throw new Error('Only the authorized candidate host on 7600 is supported');
  const credentialText = fs.readFileSync(required('WL_KEY_FILE'), 'utf8').trim();
  const credentials = [...new Set(credentialText.match(/\bsk-[A-Za-z0-9_-]+/g) || [])];
  const secret = credentials.length === 1 ? credentials[0] : (!credentialText.includes('\n') && credentials.length === 0 ? credentialText : null);
  if (!secret) throw new Error('Secret file must identify exactly one gateway credential');
  const expected = { parentPid: Number(required('WL_PARENT_PID')), parentExe: fs.realpathSync(required('WL_PARENT_EXE')),
    parentArg: required('WL_PARENT_ARG'), workerExe: fs.realpathSync(required('WL_WORKER_EXE')),
    workerArg: required('WL_WORKER_ARG'), workerDigest: required('WL_WORKER_SHA256') };
  const candidateSha = required('WL_CANDIDATE_SHA');
  const codex = process.env.WL_CODEX || 'codex';
  const version = execFileSync(codex, ['--version'], { encoding: 'utf8', maxBuffer: 4096 }).trim();
  if (version !== 'codex-cli 0.155.1') throw new Error(`Unsupported installed Codex version ${version}`);
  fs.mkdirSync(out, { recursive: true, mode: 0o700 });
  const privateHome = path.join(out, 'codex-home'); fs.mkdirSync(privateHome, { mode: 0o700 });
  const redact = (value) => JSON.stringify(value).split(secret).join('[REDACTED]')
    .replace(/Bearer\s+[^\s"\\]+/gi, 'Bearer [REDACTED]').replace(/sk-[A-Za-z0-9._-]{8,}/g, '[REDACTED]');
  const events = []; const started = process.hrtime.bigint();
  const now = () => Number(process.hrtime.bigint() - started) / 1e6;
  const record = (value) => {
    const safe = JSON.parse(redact({ atMs: now(), ...value })); events.push(safe);
    fs.appendFileSync(path.join(out, 'timeline.jsonl'), `${JSON.stringify(safe)}\n`, { mode: 0o600 });
  };
  const save = (file, value) => fs.writeFileSync(path.join(out, file), `${redact(value)}\n`, { mode: 0o600 });
  const env = { PATH: process.env.PATH, HOME: privateHome, CODEX_HOME: privateHome,
    WL_GATEWAY_KEY: secret, LANG: 'C.UTF-8', NO_PROXY: '127.0.0.1,localhost,::1', no_proxy: '127.0.0.1,localhost,::1' };
  // Neither the secret nor the user's default configuration is written to disk.
  fs.writeFileSync(path.join(privateHome, 'config.toml'), `model = "gpt-6-luna"\nmodel_provider = "candidate"\nmodel_reasoning_effort = "max"\napproval_policy = "never"\nsandbox_mode = "read-only"\n[features]\nmulti_agent = true\n[model_providers.candidate]\nname = "Candidate gateway"\nbase_url = "${baseUrl}"\nenv_key = "WL_GATEWAY_KEY"\nwire_api = "responses"\nrequest_max_retries = 0\nstream_max_retries = 0\n`, { mode: 0o600 });
  execFileSync(codex, ['app-server', 'generate-json-schema', '--experimental', '--out', path.join(out, 'protocol-schema')],
    { env, stdio: 'pipe', maxBuffer: 1024 * 1024 });
  const scopes = execFileSync('rg', ['--files', 'api/crates', 'api/apps', 'web', 'scripts/node'],
    { cwd: workspace, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 }).trim().split('\n')
    .filter((name) => /\.(rs|tsx?|jsx?|cjs|mjs)$/.test(name));
  if (scopes.length < 10) throw new Error('Insufficient real source audit inventory');
  const sentinel = `WL_SENTINEL_${crypto.randomUUID()}`;
  const nonce = `WL_STEER_${crypto.randomUUID()}`;
  const sentinelFile = path.join(out, 'sentinel.txt'); fs.writeFileSync(sentinelFile, sentinel, { mode: 0o600 });
  const meta = { candidateSha, expected, candidateExecutableDigest: snapshot(expected.parentPid).digest,
    codexVersion: version, workspace, baseUrl, model: 'gpt-6-luna', reasoning: 'max', nonce, startedUtc: new Date().toISOString() };
  save('manifest.json', meta);
  const child = spawn(codex, ['app-server', '--stdio'], { cwd: workspace, env, stdio: ['pipe', 'pipe', 'pipe'] });
  child.stderr.on('data', (data) => record({ kind: 'client/stderr', text: data.toString() }));
  const rpc = new Rpc(child, record);
  const terminal = (id) => rpc.wait((e) => e.method === 'turn/completed' && e.params.threadId === meta.threadId && e.params.turn.id === id);
  const startTurn = async (prompt) => {
    const result = await rpc.request('turn/start', { threadId: meta.threadId, input: textInput(prompt), model: 'gpt-6-luna', effort: 'max' });
    record({ kind: 'turn/invoked', turnId: result.turn.id }); return result.turn.id;
  };
  const semantic = (id, since, onMatch) => rpc.wait((e) => e.params?.threadId === meta.threadId && e.params?.turnId === id
    && ['item/agentMessage/delta', 'item/reasoning/summaryTextDelta', 'item/reasoning/textDelta'].includes(e.method)
    && typeof e.params.delta === 'string' && e.params.delta.trim().length > 0, 900000, since, onMatch);
  const prompt = (index, extra = '') => {
    const dimensions = ['resource ownership and cancellation', 'state transitions and invariants', 'error propagation and recovery',
      'algorithmic complexity and edge cases', 'field contracts and data integrity', 'test gaps and executable counterexamples'];
    const scope = scopes[index % scopes.length];
    return `Continue the same real read-only source audit. Work unit ${index}: examine ${scope} and its direct callers for ${dimensions[Math.floor(index / scopes.length) % dimensions.length]}. Read actual source with capped rg/sed tool output, trace one concrete nontrivial path, and produce a useful evidence-based report with file/line citations, counterexamples, and a proposed verification algorithm. Never sleep, run waits to fill time, emit empty work, call network/auth endpoints, install, build, start servers, or modify source/files. Use read-only shell tools. Artifacts are captured by the client. ${extra}`;
  };
  const commandProof = (id) => events.filter((e) => e.method === 'item/completed' && e.params.threadId === meta.threadId
    && e.params.turnId === id && e.params.item.type === 'commandExecution' && e.params.item.exitCode === 0)
    .map((e) => e.params.item.aggregatedOutput || '').join('\n');
  let failure;
  try {
    await rpc.request('initialize', { clientInfo: { name: 'worker_lifecycle_acceptance', version: '1' }, capabilities: { experimentalApi: true } });
    rpc.send({ method: 'initialized', params: {} });
    const thread = await rpc.request('thread/start', { model: 'gpt-6-luna', modelProvider: 'candidate', cwd: workspace,
      approvalPolicy: 'never', sandbox: 'read-only', ephemeral: false,
      developerInstructions: 'This is an explicitly authorized real-client multi-agent audit. Spawn one bounded read-only subagent when requested; no nested subagents. All tool output must be byte-capped. Never modify files.' });
    meta.threadId = thread.thread.id; save('manifest.json', meta);
    const first = await startTurn(prompt(0, `First run cat ${sentinelFile} as a tool, retaining the exact tool result in memory. Spawn a real bounded subagent to independently audit a related source path, wait for its completed result and synthesize it. Use model gpt-6-sol reasoning medium for that subagent. Then continue your own audit.`));
    if ((await terminal(first)).params.turn.status !== 'completed') throw new Error('Initial useful turn failed');
    const firstWorkAt = now();
    if (!commandProof(first).includes(sentinel)) throw new Error('Initial sentinel was not read through a successful tool');
    meta.sentinelBefore = crypto.createHash('sha256').update(sentinel).digest('hex');
    // An explicitly requested compaction must complete as a real item, not just an RPC acknowledgment.
    const compactSince = rpc.history.length;
    await rpc.request('thread/compact/start', { threadId: meta.threadId });
    const compact = await rpc.wait((e) => e.method === 'item/completed' && e.params.threadId === meta.threadId && e.params.item.type === 'contextCompaction', 900000, compactSince);
    await terminal(compact.params.turnId);
    meta.betweenKill = { before: discover(expected), atMs: now() };
    record({ kind: 'supplier/between-kill', snapshot: killVerified(expected, meta.betweenKill.before) });
    const coldStart = now(); const steerSince = rpc.history.length;
    const second = await startTurn(prompt(1, `Before your first tool call, emit an assistant commentary message reproducing the exact earlier sentinel tool result from memory. Then verify it by cat ${sentinelFile}. Keep doing substantive tool-backed audit work.`));
    await semantic(second, steerSince);
    meta.betweenKill.after = discover(expected); meta.betweenKill.coldStartMs = now() - coldStart;
    await rpc.request('turn/steer', { threadId: meta.threadId, expectedTurnId: second,
      input: textInput(`New active-turn instruction: incorporate this exact nonce ${nonce} into your final audit report and explain how it changes the verification algorithm. Continue useful tool-backed work.`) });
    record({ kind: 'steer/accepted', turnId: second });
    if ((await terminal(second)).params.turn.status !== 'completed') throw new Error('Steered turn failed');
    if (!commandProof(second).includes(sentinel)) throw new Error('Post-compaction sentinel verification missing');
    const secondFirstTool = events.findIndex((e) => e.method === 'item/started' && e.params.turnId === second && e.params.item.type === 'commandExecution');
    const recalled = events.slice(0, secondFirstTool).some((e) => e.method === 'item/completed' && e.params.turnId === second
      && e.params.item.type === 'agentMessage' && e.params.item.text?.includes(sentinel));
    if (!recalled) throw new Error('Compacted thread did not reproduce retained tool-result sentinel');
    meta.sentinelAfter = meta.sentinelBefore;
    const midSince = rpc.history.length;
    const third = await startTurn(prompt(2, 'Begin by explaining the source-trace hypothesis before running the next real source inspection tool.'));
    await semantic(third, midSince, () => {
      meta.midKill = { turnId: third, semanticSeen: true, before: discover(expected), atMs: now() };
      record({ kind: 'supplier/midcall-kill', snapshot: killVerified(expected, meta.midKill.before), turnId: third });
    });
    if ((await terminal(third)).params.turn.status !== 'failed') throw new Error('Midcall supplier death did not produce terminal failure; hidden retry/replay cannot pass');
    // One lawful new invocation follows failure; never retry the failed invocation.
    const recoverySince = rpc.history.length; const recoveryStart = now();
    const recovery = await startTurn(prompt(3, 'The prior invocation terminally failed after an intentional supplier death. This is a new lawful source-audit work unit, do not replay prior work.'));
    await semantic(recovery, recoverySince); meta.midKill.after = discover(expected); meta.midKill.coldStartMs = now() - recoveryStart;
    if ((await terminal(recovery)).params.turn.status !== 'completed') throw new Error('Lawful recovery turn failed');
    let index = 4;
    while (now() - firstWorkAt < 5400000) {
      if (index >= scopes.length * 6) throw new Error('Exhausted genuine audit work inventory before minimum duration');
      const id = await startTurn(prompt(index++));
      if ((await terminal(id)).params.turn.status !== 'completed') throw new Error('Unplanned work turn failure');
      record({ kind: 'supplier/snapshot', snapshot: discover(expected), workUnit: index });
      save('progress.json', { ...meta, durationMs: now(), workUnits: index });
    }
  } catch (error) { failure = error.message; record({ kind: 'fixture/failure', error: failure }); }
  finally {
    meta.durationMs = now(); meta.finishedUtc = new Date().toISOString(); meta.failure = failure || null;
    const verdict = evaluate(events, meta); if (failure) { verdict.pass = false; verdict.errors.push(failure); }
    save('manifest.json', meta); save('verdict.json', verdict);
    child.stdin.end(); child.kill('SIGTERM');
    // Only the fixture's private client process is reaped; candidate API remains running.
    const forceTimer = setTimeout(() => { if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL'); }, 5000);
    await new Promise((resolve) => { if (child.exitCode !== null || child.signalCode !== null) resolve(); else child.once('exit', resolve); });
    clearTimeout(forceTimer);
    console.log(JSON.stringify({ pass: verdict.pass, usefulTurns: verdict.usefulTurns, errors: verdict.errors, artifacts: out }));
    process.exitCode = verdict.pass ? 0 : 1;
  }
}
main().catch((error) => { console.error(`Fixture setup failed: ${error.message}`); process.exitCode = 1; });
