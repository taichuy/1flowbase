'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { assertNoArtifactSecrets } = require('../cli-smoke/artifact-scan');
const { promptForTurn, TEXT_PROMPT, TEXT_SENTINEL, TOOL_SENTINEL } = require('../cli-smoke/invocations');
const { readReadyManifest } = require('../cli-smoke/inputs');
const { loadPinnedInventory } = require('../wire-audit/inventory');
const { runWireAudit } = require('../wire-audit/runner');
const { clientPlan } = require('./drivers');
const { runAcpClient } = require('./harness');
const { CLIENT_NAMES, loadLock } = require('./lock');
const { redact } = require('./redact');
const { resolveClients } = require('./resolver');

const NEW_PROMPT_SENTINEL = '1flowbase gateway new prompt sentinel ok';

function prepareEvidenceRoot(root) {
  const resolved = path.resolve(root);
  fs.rmSync(resolved, { recursive: true, force: true });
  fs.mkdirSync(resolved, { recursive: true, mode: 0o700 });
  return resolved;
}

function writeJson(filePath, value, secrets = []) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true, mode: 0o700 });
  fs.writeFileSync(filePath, `${JSON.stringify(redact(value, secrets), null, 2)}\n`, { mode: 0o600 });
}

function temporaryClientPaths(client) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), `1flowbase-acp-${client}-`));
  const paths = { root, home: path.join(root, 'home'), config: path.join(root, 'config'), output: path.join(root, 'output') };
  for (const directory of [paths.home, paths.config, paths.output]) fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
  return paths;
}

async function readJson(endpoint, fetchImpl) {
  const response = await fetchImpl(endpoint.url, { method: endpoint.method ?? 'GET', headers: endpoint.headers ?? {} });
  if (!response.ok) throw new Error(`compatibility evidence endpoint returned HTTP ${response.status}`);
  return response.json();
}

function listRunIds(payload) {
  const data = payload && typeof payload === 'object' && 'data' in payload ? payload.data : payload;
  return new Set((Array.isArray(data?.items) ? data.items : []).map((item) => item?.id).filter(Boolean));
}

async function queryRuns(target, runIds, fetchImpl) {
  return Promise.all(runIds.map(async (runId) => {
    const endpoint = {
      ...target.durable.query_run,
      url: target.durable.query_run.url_template.replace('{run_id}', runId),
    };
    const payload = await readJson(endpoint, fetchImpl);
    const data = payload && typeof payload === 'object' && 'data' in payload ? payload.data : payload;
    return {
      id: data?.id ?? null,
      status: data?.status ?? null,
      answer: data?.answer ?? null,
      usage: data?.usage ?? null,
    };
  }));
}

function assertDurableTurns(client, runs) {
  const expectedAnswers = [TEXT_SENTINEL, TOOL_SENTINEL, NEW_PROMPT_SENTINEL];
  const matched = expectedAnswers.map((sentinel) => runs.find((run) => run.answer?.includes(sentinel)));
  if (matched.some((run) => !run)) throw new Error(`${client} durable runs omitted an expected prompt answer`);
  if (new Set(matched.map((run) => run.id)).size !== 3) throw new Error(`${client} durable prompt runs reused an id`);
  if (matched.some((run) => run.status !== 'succeeded')) throw new Error(`${client} durable prompt run was not succeeded`);
  if (matched.some((run) => !run.usage)) throw new Error(`${client} durable prompt run omitted usage`);
  if (new Set(matched.map((run) => JSON.stringify(run.usage))).size !== 3) {
    throw new Error(`${client} durable prompt runs reused usage`);
  }
  return matched;
}

function assertClientResult(client, result, providerEvents) {
  if (result.turns.length !== 3 || result.turns.some((turn) => typeof turn.stop_reason !== 'string')) {
    throw new Error(`${client} ACP lifecycle did not complete three prompts`);
  }
  if (!result.turns[0].text.includes(TEXT_SENTINEL)) throw new Error(`${client} ACP text sentinel was not observed`);
  if (!result.turns[1].text.includes(TOOL_SENTINEL)) throw new Error(`${client} ACP tool sentinel was not observed`);
  if (!result.turns[2].text.includes(NEW_PROMPT_SENTINEL)) throw new Error(`${client} ACP new prompt response was not observed`);
  if (!result.turns[1].tools.some((tool) => tool.status === 'completed')) {
    throw new Error(`${client} ACP tool did not reach completed`);
  }
  const timeline = result.timeline.map((entry) => entry.event);
  const required = ['tool_call', 'barrier_release_start', 'barrier_released', 'prompt_terminal'];
  for (const event of required) if (!timeline.includes(event)) throw new Error(`${client} ACP timeline omitted ${event}`);
  const first = result.timeline.findIndex((entry) => entry.event === 'text_delta' && entry.update?.text?.includes('marker-1'));
  const release = timeline.indexOf('barrier_release_start');
  const second = result.timeline.findIndex((entry) => entry.event === 'text_delta' && entry.update?.text?.includes('marker-2'));
  const terminal = timeline.lastIndexOf('prompt_terminal');
  if (first < 0 || release <= first || second <= release || terminal <= second) {
    throw new Error(`${client} ACP timeline did not prove marker barrier chronology`);
  }
  const producerOrder = ['tool_call', 'second_upstream_request'];
  const positions = producerOrder.map((event) => providerEvents.findIndex((entry) => entry.event === event));
  if (positions.some((position) => position < 0) || positions[1] <= positions[0]) {
    throw new Error(`${client} provider timeline did not prove tool result continuation`);
  }
}

async function runClientCompatibility(rawOptions, dependencies = {}) {
  const fetchImpl = dependencies.fetchImpl ?? globalThis.fetch;
  const manifest = readReadyManifest(rawOptions.readyManifest);
  if (!manifest.controlledUpstream) throw new Error('ACP compatibility requires controlled upstream endpoints');
  const lock = loadLock(rawOptions.lockPath);
  const resolved = resolveClients(lock, rawOptions.runtimeRoot);
  const evidenceRoot = prepareEvidenceRoot(rawOptions.evidenceRoot);
  const secretCanary = rawOptions.secretCanary ?? 'sk-1flowbase-controlled-secret-canary';
  const secrets = [manifest.openai.api_key, manifest.anthropic.api_key, secretCanary];
  const clients = {};
  const paths = [];
  try {
    writeJson(path.join(evidenceRoot, 'provenance.json'), { lock, resolved }, secrets);
    writeJson(path.join(evidenceRoot, 'wire-inventory.json'), loadPinnedInventory(), secrets);
    const wireAudit = await (dependencies.runWireAudit ?? runWireAudit)({ manifest }, { fetchImpl, secretCanary });
    writeJson(path.join(evidenceRoot, 'wire-audit.json'), wireAudit, secrets);

    for (const name of CLIENT_NAMES) {
      const clientPaths = temporaryClientPaths(name);
      paths.push(clientPaths.root);
      const toolPath = path.join(clientPaths.output, 'tool-vector.txt');
      fs.writeFileSync(toolPath, `1flowbase-client-tool-result\n${secretCanary}\n`, { mode: 0o600 });
      const target = name === 'claude' ? manifest.anthropic : manifest.openai;
      const beforeRuns = listRunIds(await readJson(target.durable.list_runs, fetchImpl));
      const beforeProvider = await readJson({ url: manifest.controlledUpstream.snapshotUrl }, fetchImpl);
      const plan = clientPlan(name, resolved[name], dependencies.parentEnv ?? process.env, clientPaths, manifest.gatewayBaseUrl, target);
      plan.secrets = [...plan.secrets, secretCanary];
      const prompts = [
        TEXT_PROMPT,
        promptForTurn('tool', clientPaths),
        `This is a new prompt after the completed tool turn. Reply with exactly: ${NEW_PROMPT_SENTINEL}. ${TEXT_PROMPT}`,
      ];
      const result = await (dependencies.runAcpClient ?? runAcpClient)(plan, {
        prompts,
        releaseOnMarkers: ['marker-1'],
        onMarker: async () => {
          const response = await fetchImpl(manifest.controlledUpstream.barrierReleaseUrl, { method: 'POST' });
          if (!response.ok) throw new Error(`controlled barrier release returned HTTP ${response.status}`);
        },
      });
      const afterProvider = await readJson({ url: manifest.controlledUpstream.snapshotUrl }, fetchImpl);
      const cursor = beforeProvider.entries?.at(-1)?.sequence ?? 0;
      const providerEvents = (afterProvider.entries ?? []).filter((entry) => entry.sequence > cursor);
      assertClientResult(name, result, providerEvents);
      const afterRuns = listRunIds(await readJson(target.durable.list_runs, fetchImpl));
      const newRunIds = [...afterRuns].filter((id) => !beforeRuns.has(id));
      if (newRunIds.length < 3 || new Set(newRunIds).size !== newRunIds.length) {
        throw new Error(`${name} expected at least three distinct durable runs, received ${newRunIds.length}`);
      }
      const durableRuns = await queryRuns(target, newRunIds, fetchImpl);
      const promptRuns = assertDurableTurns(name, durableRuns);
      clients[name] = {
        ...result,
        durable_run_ids: promptRuns.map((run) => run.id),
        durable_runs: durableRuns,
        provider_events: providerEvents,
      };
      writeJson(path.join(evidenceRoot, 'clients', `${name}.json`), clients[name], secrets);
    }

    const snapshot = await readJson({ url: manifest.controlledUpstream.snapshotUrl }, fetchImpl);
    if (snapshot.counters?.gatewayExecutorInvocations !== 0) throw new Error('gateway executor observer recorded an invocation');
    if (snapshot.counters?.networkObserverOutbound !== 0) throw new Error('gateway connected to controlled network observer');
    writeJson(path.join(evidenceRoot, 'controlled-upstream.json'), snapshot, secrets);
    const scanned = assertNoArtifactSecrets([evidenceRoot], secrets);
    const result = {
      schema_version: '1flowbase.acp-client-compatibility-result/v1',
      status: 'pass',
      clients: Object.fromEntries(Object.entries(clients).map(([name, value]) => [name, {
        turns: value.turns.length, durable_runs: value.durable_run_ids.length, timeline_events: value.timeline.length,
      }])),
      wire_audit: wireAudit,
      scanned_artifact_count: scanned.length,
      evidence_root: evidenceRoot,
    };
    writeJson(path.join(evidenceRoot, 'result.json'), result, secrets);
    return result;
  } catch (error) {
    writeJson(path.join(evidenceRoot, 'result.json'), {
      schema_version: '1flowbase.acp-client-compatibility-result/v1',
      status: 'fail',
      error: { name: error.name, message: error.message },
      evidence_root: evidenceRoot,
    }, secrets);
    assertNoArtifactSecrets([evidenceRoot], secrets);
    throw error;
  } finally {
    for (const root of paths) fs.rmSync(root, { recursive: true, force: true });
  }
}

module.exports = {
  NEW_PROMPT_SENTINEL,
  assertClientResult,
  assertDurableTurns,
  listRunIds,
  queryRuns,
  runClientCompatibility,
  temporaryClientPaths,
};
