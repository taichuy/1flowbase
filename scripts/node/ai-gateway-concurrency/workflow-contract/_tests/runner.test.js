'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { TRANSPORT } = require('../../contracts');
const { readReadyManifest, runWorkflowContract } = require('../runner');

function fixtureInputs() {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-runner-'));
  const executable = path.join(repoRoot, 'executable');
  fs.writeFileSync(executable, '#!/bin/sh\n');
  fs.chmodSync(executable, 0o755);
  const openaiPackageDir = path.join(repoRoot, 'openai');
  const anthropicPackageDir = path.join(repoRoot, 'anthropic');
  fs.mkdirSync(openaiPackageDir);
  fs.mkdirSync(anthropicPackageDir);
  fs.writeFileSync(path.join(openaiPackageDir, 'openai.1flowbasepkg'), 'openai');
  fs.writeFileSync(path.join(anthropicPackageDir, 'anthropic.1flowbasepkg'), 'anthropic');
  return {
    repoRoot,
    mainSourceSha: 'a'.repeat(40),
    officialSourceSha: 'b'.repeat(40),
    profile: 'characterize',
    databaseUrl: 'postgres://postgres:password@127.0.0.1:5432/fixture',
    apiServerBin: executable,
    pluginRunnerBin: executable,
    openaiPackageDir,
    anthropicPackageDir,
    codexExecutable: executable,
    claudeExecutable: executable,
    hostTarget: 'x86_64-unknown-linux-gnu',
  };
}

function fixtureManifest() {
  const durable = (provider) => ({
    query_run: {
      method: 'GET',
      url_template: `http://127.0.0.1:4100/api/agent/v1/runs/{run_id}`,
      headers: { authorization: `Bearer ${provider}-application-key` },
    },
    list_runs: {
      method: 'GET',
      url: `http://127.0.0.1:4100/api/console/applications/${provider}/logs/runs?page=1&page_size=100&cache_mode=bypass`,
      headers: { cookie: 'fixture-owner' },
    },
  });
  const activity = (provider) => ({
    method: 'GET',
    url: `http://127.0.0.1:4100/api/console/applications/${provider}/monitoring/runtime-activity`,
    headers: { cookie: 'fixture-owner' },
  });
  const streams = { method: 'GET', url: 'http://127.0.0.1:4200/providers/active-streams' };
  return {
    schema_version: '1flowbase.ai-gateway-fixture/v1',
    targets: {
      openai: {
        api_key: 'openai-application-key',
        model: 'published-openai-model',
        gateway: { responses_url: 'http://127.0.0.1:4100/v1/responses' },
        durable: durable('openai'),
        runtime_activity: activity('openai'),
        plugin_runner_active_streams: streams,
      },
      anthropic: {
        api_key: 'anthropic-application-key',
        model: 'published-anthropic-model',
        gateway: { anthropic_messages_url: 'http://127.0.0.1:4100/v1/messages' },
        durable: durable('anthropic'),
        runtime_activity: activity('anthropic'),
        plugin_runner_active_streams: streams,
      },
    },
  };
}

test('AC-003/006/007: runner orders WP1/WP3/WP4/WP2F and forwards distinct ready-manifest keys once', async () => {
  const inputs = fixtureInputs();
  const calls = [];
  const result = await runWorkflowContract(inputs, {
    createMockUpstream() {
      calls.push('mock:create');
      return {
        async start() { calls.push('mock:start'); return { httpBaseUrl: 'http://127.0.0.1:4000', websocketBaseUrl: 'ws://127.0.0.1:4000' }; },
        snapshot() { return { active: 0, peak: 1, arrivals: 0, entries: [] }; },
        async stop() { calls.push('mock:stop'); },
      };
    },
    async createGatewayFixture(options) {
      calls.push(['fixture:create', options.upstreamBaseUrl]);
      return { result: fixtureManifest(), async close() { calls.push('fixture:close'); } };
    },
    async runCliSmoke(options) {
      calls.push(['smoke', fs.existsSync(options.readyManifest)]);
      return { status: 'pass', codex_event_count: 2, claude_event_count: 3 };
    },
    async runGatewayCharacterize(options) {
      calls.push(['characterize', options]);
      return {
        summary: {
          verdict: 'PASS',
          totals: { requests: 180, contractFailures: 0 },
          durableConvergence: { verdict: 'PASS', requests: 120, polls: 2 },
        },
        artifacts: { outputDirectory: path.join(inputs.repoRoot, 'tmp/test-governance/ai-gateway-concurrency') },
      };
    },
  });
  assert.equal(result.status, 'pass');
  assert.deepEqual(result.characterize.durable_convergence, { verdict: 'PASS', requests: 120, polls: 2 });
  assert.deepEqual(calls.map((call) => Array.isArray(call) ? call[0] : call), [
    'mock:create', 'mock:start', 'fixture:create', 'smoke', 'characterize', 'fixture:close', 'mock:stop',
  ]);
  const characterize = calls.find((call) => Array.isArray(call) && call[0] === 'characterize')[1];
  assert.deepEqual(characterize.authorizationTokenByTransport, {
    [TRANSPORT.RESPONSES_SSE]: 'openai-application-key',
    [TRANSPORT.ANTHROPIC_SSE]: 'anthropic-application-key',
  });
  assert.deepEqual(characterize.modelByTransport, {
    [TRANSPORT.RESPONSES_SSE]: 'published-openai-model',
    [TRANSPORT.ANTHROPIC_SSE]: 'published-anthropic-model',
  });
  assert.equal(characterize.endpointSet[TRANSPORT.RESPONSES_WEBSOCKET], 'ws://127.0.0.1:4000/v1/responses');
  assert.deepEqual(
    characterize.durableTargetsByTransport[TRANSPORT.RESPONSES_SSE],
    fixtureManifest().targets.openai,
  );
  assert.equal(JSON.stringify(result).includes('application-key'), false);
  assert.equal(fs.existsSync(path.join(inputs.repoRoot, 'tmp/test-governance/ai-gateway-concurrency/gateway-ready.json')), false);
});

test('AC-007 controlled negative: runner still closes owned fixture and mock after sentinel failure', async () => {
  const inputs = fixtureInputs();
  const calls = [];
  const result = await runWorkflowContract(inputs, {
    createMockUpstream() {
      return {
        async start() { return { httpBaseUrl: 'http://127.0.0.1:4000', websocketBaseUrl: 'ws://127.0.0.1:4000' }; },
        snapshot() { return { active: 0, peak: 0, arrivals: 0, entries: [] }; },
        async stop() { calls.push('mock:stop'); },
      };
    },
    async createGatewayFixture() {
      return { result: fixtureManifest(), async close() { calls.push('fixture:close'); } };
    },
    async runCliSmoke() { throw new Error('sentinel failed with openai-application-key'); },
  });
  assert.equal(result.status, 'fail');
  assert.deepEqual(calls, ['fixture:close', 'mock:stop']);
  assert.equal(result.error.message.includes('openai-application-key'), false);
  assert.match(result.error.message, /<redacted>/u);
});

test('AC-003 controlled negative: ready target tuple requires endpoint, key, and published model', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-ready-'));
  const readyFile = path.join(root, 'ready.json');
  const manifest = fixtureManifest();
  delete manifest.targets.anthropic.model;
  fs.writeFileSync(readyFile, JSON.stringify(manifest));
  assert.throws(() => readReadyManifest(readyFile), /omitted anthropic published model/u);
  const invalidEndpoint = fixtureManifest();
  invalidEndpoint.targets.openai.gateway.responses_url = 'https://provider.example/v1/responses';
  fs.writeFileSync(readyFile, JSON.stringify(invalidEndpoint));
  assert.throws(() => readReadyManifest(readyFile), /credential-free loopback/u);
});
