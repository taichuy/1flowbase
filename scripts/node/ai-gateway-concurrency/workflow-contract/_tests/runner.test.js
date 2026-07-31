'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { TRANSPORT } = require('../../contracts');
const { protocolOracleInventory, readReadyManifest, runWorkflowContract } = require('../runner');

function fixtureInputs() {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-runner-'));
  const executable = path.join(repoRoot, 'executable');
  fs.writeFileSync(executable, '#!/bin/sh\n');
  fs.chmodSync(executable, 0o755);
  const openaiPackageDir = path.join(repoRoot, 'openai');
  const anthropicPackageDir = path.join(repoRoot, 'anthropic');
  const openaiCompatiblePackageDir = path.join(repoRoot, 'openai_compatible');
  fs.mkdirSync(openaiPackageDir);
  fs.mkdirSync(anthropicPackageDir);
  fs.mkdirSync(openaiCompatiblePackageDir);
  fs.writeFileSync(path.join(openaiPackageDir, 'openai.1flowbasepkg'), 'openai');
  fs.writeFileSync(path.join(anthropicPackageDir, 'anthropic.1flowbasepkg'), 'anthropic');
  fs.writeFileSync(
    path.join(openaiCompatiblePackageDir, 'openai_compatible.1flowbasepkg'),
    'openai-compatible'
  );
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
    openaiCompatiblePackageDir,
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
  const target = (provider, ordinal, model, gateway) => ({
    application_id: `${provider}-application-${ordinal}`,
    provider_instance_id: `${provider}-instance-${ordinal}`,
    publication_id: `${provider}-publication-${ordinal}`,
    api_key: `${provider}-application-key-${ordinal}`,
    model,
    upstream_model: 'gateway-fixture-model',
    gateway,
    durable: durable(`${provider}-${ordinal}`),
    runtime_activity: activity(`${provider}-${ordinal}`),
    plugin_runner_active_streams: streams,
  });
  const openai = target('openai', 1, 'published-openai-model', { responses_url: 'http://127.0.0.1:4100/v1/responses' });
  const openaiCompatible = target(
    'openai_compatible',
    1,
    'published-compatible-model',
    { chat_completions_url: 'http://127.0.0.1:4100/v1/chat/completions' }
  );
  const anthropicPool = [1, 2].map((ordinal) => target(
    'anthropic', ordinal, 'published-anthropic-model', { anthropic_messages_url: 'http://127.0.0.1:4100/v1/messages' }
  ));
  return {
    schema_version: '1flowbase.ai-gateway-fixture/v1',
    targets: {
      openai,
      openai_compatible: openaiCompatible,
      anthropic: anthropicPool[0],
    },
    pools: { anthropic: anthropicPool },
  };
}

test('Root #1477 AC-001/004/005/006/008/009: workflow invokes the complete deterministic oracle inventory', () => {
  const inventory = protocolOracleInventory();
  assert.equal(inventory.rows, 16);
  assert.equal(inventory.request_fidelity.positive_rows.length, 3);
  assert.equal(inventory.request_fidelity.negative_rows.length, 2);
  assert.equal(inventory.request_fidelity.translation_rows.length, 1);
  assert.equal(inventory.protocol_context_profiles.rows, 9);
  assert.deepEqual(inventory.protocol_context_profiles.sources, [
    'anthropic_messages', 'openai_chat', 'openai_responses',
  ]);
  assert.deepEqual(inventory.protocol_context_profiles.providers, [
    'anthropic', 'openai', 'openai_compatible',
  ]);
  assert.equal(inventory.error_fidelity.rows, 20);
  assert.deepEqual(inventory.canonical_stream_regression.partitions, ['whole', 'bytewise', 'uneven']);
  assert.equal(inventory.canonical_stream_regression.successTerminalCount, 1);
  assert.equal(inventory.canonical_stream_regression.durableParity.preservesRepeatedContent, true);
  assert.deepEqual(inventory.anthropic_callback_retry, {
    vector_id: 'tools-callback-retry-after-429',
    provider_outcomes: ['completed', 'http-429', 'completed'],
    durable_statuses: ['failed', 'succeeded'],
    client_tool_results: 1,
  });
  assert.deepEqual(inventory.provenance.providers, [
    'openai', 'anthropic', 'openai_compatible',
  ]);
});

test('AC-003/006/007: runner orders WP1/WP3/WP4/WP2F and forwards distinct ready-manifest keys once', async () => {
  const inputs = fixtureInputs();
  const staleArtifact = path.join(inputs.repoRoot, 'tmp/test-governance/ai-gateway-concurrency/stale-secret.json');
  fs.mkdirSync(path.dirname(staleArtifact), { recursive: true });
  fs.writeFileSync(staleArtifact, 'stale secret from a prior cycle');
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
      assert.equal(fs.existsSync(staleArtifact), false);
      calls.push(['fixture:create', options.upstreamBaseUrl, options.artifactRoot]);
      return {
        result: fixtureManifest(),
        async close() {
          assert.equal(
            fs.existsSync(path.join(inputs.repoRoot, 'tmp/test-governance/ai-gateway-concurrency/gateway-ready.json')),
            false,
          );
          calls.push('fixture:close');
        },
      };
    },
    async verifyRuntimeProvenance() {
      calls.push('runtime-provenance');
      return { verdict: 'PASS', providers: {} };
    },
    async verifyGatewayRequestFidelity() {
      calls.push('request-fidelity');
      return { verdict: 'PASS', rows: [] };
    },
    async verifyProtocolContextProfileMatrix() {
      calls.push('protocol-context-profiles');
      return { verdict: 'PASS', rows: [] };
    },
    async verifyAnthropicCallbackRetry() {
      calls.push('anthropic-callback-retry');
      return { verdict: 'PASS', vector_id: 'tools-callback-retry-after-429' };
    },
    async runWireAudit(options) {
      calls.push(['wire-audit', options.manifest.gatewayBaseUrl]);
      return { counters: { gateway_executor_invocations: 0, network_observer_outbound: 0 } };
    },
    async runGatewayWebSocketAcceptance() {
      calls.push('responses-websocket');
      return { trace: { terminal_count: 1 }, durable: { run: { status: 'succeeded' } }, wire_audit: { verdict: 'PASS' } };
    },
    async runGatewayCharacterize(options) {
      calls.push(['characterize', options]);
      return {
        summary: {
          verdict: 'PASS',
          totals: {
            requests: 225,
            blockingRequests: 29,
            advisoryRequests: 196,
            contractFailures: 0,
            advisoryFailures: 2,
          },
          durableConvergence: {
            verdict: 'PASS', requests: 14, polls: 8, rows: 8, observabilityAdvisories: 2,
          },
        },
        artifacts: { outputDirectory: path.join(inputs.repoRoot, 'tmp/test-governance/ai-gateway-concurrency') },
      };
    },
  });
  assert.equal(result.status, 'pass');
  assert.deepEqual(result.characterize.durable_convergence, {
    verdict: 'PASS', requests: 14, polls: 8, rows: 8, observabilityAdvisories: 2,
  });
  assert.equal(result.characterize.blocking_requests, 29);
  assert.equal(result.characterize.performance_requests, 196);
  assert.equal(result.characterize.performance_and_observability_advisories, 2);
  assert.deepEqual(calls.map((call) => Array.isArray(call) ? call[0] : call), [
    'mock:create', 'mock:start', 'fixture:create', 'runtime-provenance', 'request-fidelity',
    'protocol-context-profiles', 'anthropic-callback-retry', 'wire-audit',
    'responses-websocket', 'characterize', 'fixture:close', 'mock:stop',
  ]);
  const characterize = calls.find((call) => Array.isArray(call) && call[0] === 'characterize')[1];
  assert.deepEqual(characterize.authorizationTokenByTransport, {
    [TRANSPORT.RESPONSES_SSE]: 'openai-application-key-1',
    [TRANSPORT.CHAT_COMPLETIONS_SSE]: 'openai_compatible-application-key-1',
    [TRANSPORT.ANTHROPIC_SSE]: 'anthropic-application-key-1',
  });
  assert.deepEqual(characterize.modelByTransport, {
    [TRANSPORT.RESPONSES_SSE]: 'published-openai-model',
    [TRANSPORT.CHAT_COMPLETIONS_SSE]: 'published-compatible-model',
    [TRANSPORT.ANTHROPIC_SSE]: 'published-anthropic-model',
  });
  assert.equal(characterize.endpointSet[TRANSPORT.RESPONSES_WEBSOCKET], 'ws://127.0.0.1:4000/v1/responses');
  assert.equal(characterize.endpointSet[TRANSPORT.CHAT_COMPLETIONS_SSE], 'http://127.0.0.1:4100/v1/chat/completions');
  assert.deepEqual(
    characterize.durableTargetsByTransport[TRANSPORT.RESPONSES_SSE],
    fixtureManifest().targets.openai,
  );
  assert.deepEqual(
    characterize.durableTargetsByTransport[TRANSPORT.CHAT_COMPLETIONS_SSE],
    fixtureManifest().targets.openai_compatible,
  );
  assert.deepEqual(characterize.anthropicTargetPool, fixtureManifest().pools.anthropic);
  const fixtureCreate = calls.find((call) => Array.isArray(call) && call[0] === 'fixture:create');
  assert.equal(fixtureCreate[2], path.join(inputs.repoRoot, 'tmp/test-governance/ai-gateway-concurrency'));
  assert.equal(result.targets.anthropic_pool.length, 2);
  assert.equal(JSON.stringify(result).includes('application-key'), false);
  assert.equal(fs.existsSync(path.join(inputs.repoRoot, 'tmp/test-governance/ai-gateway-concurrency/gateway-ready.json')), false);
});

test('AC-007 controlled negative: runner still closes owned fixture and mock after WireAudit failure', async () => {
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
    async verifyRuntimeProvenance() { return { verdict: 'PASS', providers: {} }; },
    async verifyGatewayRequestFidelity() { return { verdict: 'PASS', rows: [] }; },
    async verifyProtocolContextProfileMatrix() { return { verdict: 'PASS', rows: [] }; },
    async verifyAnthropicCallbackRetry() { return { verdict: 'PASS' }; },
    async runWireAudit() { throw new Error('wire audit failed with anthropic-application-key-2'); },
    async runGatewayWebSocketAcceptance() { return { wire_audit: { verdict: 'PASS' } }; },
    async runGatewayCharacterize() { throw new Error('characterize also failed'); },
  });
  assert.equal(result.status, 'fail');
  assert.deepEqual(calls, ['fixture:close', 'mock:stop']);
  assert.equal(result.error.message.includes('anthropic-application-key-2'), false);
  assert.match(result.error.message, /<redacted>/u);
  assert.deepEqual(result.protocol_conformance.failures.map((failure) => failure.name), ['wire-audit', 'characterize']);
});

test('AC service logs: cleanup persistence failure makes the workflow and cleanup fail', async () => {
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
      return {
        result: fixtureManifest(),
        async close() { calls.push('fixture:close'); throw new Error('service log persistence failed'); },
      };
    },
    async verifyRuntimeProvenance() { return { verdict: 'PASS', providers: {} }; },
    async verifyGatewayRequestFidelity() { return { verdict: 'PASS', rows: [] }; },
    async verifyProtocolContextProfileMatrix() { return { verdict: 'PASS', rows: [] }; },
    async verifyAnthropicCallbackRetry() { return { verdict: 'PASS' }; },
    async runWireAudit() { return { counters: { gateway_executor_invocations: 0, network_observer_outbound: 0 } }; },
    async runGatewayWebSocketAcceptance() { return { wire_audit: { verdict: 'PASS' } }; },
    async runGatewayCharacterize() {
      return {
        summary: {
          verdict: 'PASS',
          totals: {
            requests: 225,
            blockingRequests: 29,
            advisoryRequests: 196,
            contractFailures: 0,
            advisoryFailures: 0,
          },
        },
        artifacts: { outputDirectory: path.join(inputs.repoRoot, 'tmp/test-governance/ai-gateway-concurrency') },
      };
    },
  });
  assert.equal(result.status, 'fail');
  assert.equal(result.cleanup.status, 'fail');
  assert.match(result.error.message, /service log persistence failed/u);
  assert.deepEqual(calls, ['fixture:close', 'mock:stop']);
});

test('AC-003 controlled negative: ready target tuple requires endpoint, key, and published model', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-ready-'));
  const readyFile = path.join(root, 'ready.json');
  const manifest = fixtureManifest();
  delete manifest.targets.anthropic.model;
  fs.writeFileSync(readyFile, JSON.stringify(manifest));
  assert.throws(() => readReadyManifest(readyFile), /omitted anthropic published model/u);
  const missingUpstreamModel = fixtureManifest();
  delete missingUpstreamModel.targets.anthropic.upstream_model;
  fs.writeFileSync(readyFile, JSON.stringify(missingUpstreamModel));
  assert.throws(() => readReadyManifest(readyFile), /omitted anthropic upstream model/u);
  const invalidEndpoint = fixtureManifest();
  invalidEndpoint.targets.openai.gateway.responses_url = 'https://provider.example/v1/responses';
  fs.writeFileSync(readyFile, JSON.stringify(invalidEndpoint));
  assert.throws(() => readReadyManifest(readyFile), /credential-free loopback/u);
  const duplicateIdentity = fixtureManifest();
  duplicateIdentity.pools.anthropic[1].provider_instance_id = duplicateIdentity.pools.anthropic[0].provider_instance_id;
  fs.writeFileSync(readyFile, JSON.stringify(duplicateIdentity));
  assert.throws(() => readReadyManifest(readyFile), /reused provider_instance_id/u);
  const duplicateKey = fixtureManifest();
  duplicateKey.pools.anthropic[1].api_key = duplicateKey.pools.anthropic[0].api_key;
  fs.writeFileSync(readyFile, JSON.stringify(duplicateKey));
  assert.throws(() => readReadyManifest(readyFile), /reused api_key/u);
});
