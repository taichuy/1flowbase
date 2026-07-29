'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { runLocalAcceptance } = require('../runner');

function fixtureManifest() {
  return {
    schema_version: '1flowbase.local-ai-gateway-acceptance/v1',
    repo: {
      host: { path: '/host', revision: 'HEAD' },
      officialPlugins: { path: '/plugins', revision: 'b'.repeat(40) },
      protectedMain: { path: '/main', revision: 'c'.repeat(40) },
    },
    sources: {
      codex: { repository: '/codex', revision: 'd'.repeat(40), identity: 'github:openai/codex' },
      opencode: { repository: '/opencode', revision: 'e'.repeat(40), identity: 'github:anomalyco/opencode' },
    },
    database: { container: 'docker-db-1', image: 'postgres:16-alpine', host: '127.0.0.1', port: 35432 },
    artifacts: {
      apiServer: { path: '/bin/api-server', sha256: '1'.repeat(64) },
      pluginRunner: { path: '/bin/plugin-runner', sha256: '2'.repeat(64) },
      openaiPackage: { path: '/packages/openai', sha256: '3'.repeat(64) },
      anthropicPackage: { path: '/packages/anthropic', sha256: '4'.repeat(64) },
      openaiCompatiblePackage: {
        path: '/packages/openai-compatible',
        sha256: '8'.repeat(64),
      },
      codex: { path: '/bin/codex', sha256: '5'.repeat(64) },
      claude: { path: '/bin/claude', sha256: '6'.repeat(64) },
      claudeManifest: { path: '/package.json', sha256: '7'.repeat(64) },
      opencode: { path: '/bin/opencode', sha256: '8'.repeat(64) },
    },
    clients: {
      codex: { buildCommand: 'fixed-codex-build-claim' },
      claude: {
        packageName: '@anthropic-ai/claude-code', packageVersion: '2.1.218', packageIntegrity: 'sha512-fixed',
        installCommand: 'fixed-claude-install-claim',
      },
      opencode: { buildCommand: 'fixed-opencode-build-claim' },
    },
  };
}

function fixtureReady() {
  const target = (provider, ordinal) => ({
    application_id: `${provider}-app-${ordinal}`,
    provider_instance_id: `${provider}-provider-${ordinal}`,
    publication_id: `${provider}-publication-${ordinal}`,
    api_key: `${provider}-key-${ordinal}`,
    model: 'fixture-model',
    gateway: {
      base_url: 'http://127.0.0.1:4100',
      responses_url: 'http://127.0.0.1:4100/v1/responses',
      chat_completions_url: 'http://127.0.0.1:4100/v1/chat/completions',
      anthropic_messages_url: 'http://127.0.0.1:4100/v1/messages',
    },
    durable: { query_run: {}, list_runs: {} },
    runtime_activity: {},
    plugin_runner_active_streams: {},
  });
  const anthropic = [target('anthropic', 1), target('anthropic', 2)];
  return {
    schema_version: '1flowbase.ai-gateway-fixture/v1',
    gateway_base_url: 'http://127.0.0.1:4100',
    targets: {
      openai: target('openai', 1),
      openai_compatible: target('openai_compatible', 1),
      anthropic: anthropic[0],
    },
    pools: { anthropic },
    controlled_upstream: {
      snapshot_url: 'http://127.0.0.1:4000/__control/snapshot',
      barrier_release_url: 'http://127.0.0.1:4000/__control/barrier/release',
      network_observer_url: 'http://127.0.0.1:4000/__observer/mcp-network',
      gateway_executor_observer_url: 'http://127.0.0.1:4000/__observer/gateway-executor',
    },
  };
}

function dependencies({ failAt } = {}) {
  const calls = [];
  const cleanup = [];
  const deps = {
    loadManifest() { calls.push('manifest'); return fixtureManifest(); },
    async preflight() {
      calls.push('preflight');
      if (failAt === 'preflight') throw new Error('preflight failed');
      return {
        repositories: [{ name: 'protectedMain', path: '/main', revision: 'c'.repeat(40) }],
      };
    },
    createEvidenceRoot() { calls.push('evidence'); return '/evidence'; },
    createDatabase() {
      calls.push('database:create');
      return {
        url: 'postgres://role:password@127.0.0.1:35432/database',
        async close() { cleanup.push('database'); },
      };
    },
    async probeDatabase(url) { calls.push(['database:probe', url]); if (failAt === 'probe') throw new Error('probe failed'); },
    createMockUpstream(options) {
      calls.push(['mock:create', options]);
      return {
        async start() { calls.push('mock:start'); return { httpBaseUrl: 'http://127.0.0.1:4000', websocketBaseUrl: 'ws://127.0.0.1:4000' }; },
        snapshot() { return { active: 0, entries: [] }; },
        releaseBarrier() { calls.push('mock:release'); return 1; },
        async stop() { cleanup.push('mock'); },
      };
    },
    async createGatewayFixture(options) {
      calls.push(['fixture:create', options]);
      if (failAt === 'fixture') throw new Error('fixture failed');
      return { result: fixtureReady(), async close() { cleanup.push('fixture'); } };
    },
    writeReadyManifest() { calls.push('ready:write'); return '/evidence/ready.json'; },
    async runWireAudit() {
      calls.push('protocol:wire-audit');
      if (failAt === 'protocol') throw new Error('protocol failed');
      return { counters: { gateway_executor_invocations: 0, network_observer_outbound: 0 } };
    },
    async runGatewayCharacterize() {
      calls.push('protocol:characterize');
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
          durableConvergence: { verdict: 'PASS', rows: 8, observabilityAdvisories: 2 },
        },
      };
    },
    writeProtocolEvidence() { calls.push('protocol:evidence'); },
    async runLocalClientAcceptance(options) {
      calls.push(['clients', options]);
      return failAt === 'clients'
        ? { status: 'fail', clients: [{ name: 'codex', status: 'fail' }] }
        : { status: 'pass', clients: [{ name: 'codex', status: 'pass' }], final_reconciliation: { runtime_targets: 2 } };
    },
    writeSnapshot() { calls.push('snapshot:write'); },
    writeResult(_root, result) { calls.push(['result', result.status]); },
    async cleanupTmux() { cleanup.push('tmux'); },
  };
  return { deps, calls, cleanup };
}

test('WP-14A runs the machine client matrix while the mock-backed fixture is alive', async () => {
  const { deps, calls, cleanup } = dependencies();
  const result = await runLocalAcceptance({}, deps);
  assert.equal(result.status, 'pass', JSON.stringify(result));
  assert.equal(result.gate_role, 'mock_backed_local_client_acceptance');
  assert.equal(calls.filter((call) => call === 'database:create').length, 1);
  assert.equal(calls.filter((call) => Array.isArray(call) && call[0] === 'fixture:create').length, 1);
  const probe = calls.find((call) => Array.isArray(call) && call[0] === 'database:probe');
  const clients = calls.find((call) => Array.isArray(call) && call[0] === 'clients')[1];
  assert.equal(probe[1], 'postgres://role:password@127.0.0.1:35432/database');
  assert.equal(result.protocol.characterize.verdict, 'PASS');
  assert.equal(result.protocol.characterize.blocking_requests, 29);
  assert.equal(result.protocol.characterize.performance_requests, 196);
  assert.equal(result.protocol.characterize.performance_and_observability_advisories, 2);
  assert.equal(clients.discovery.binaries.opencode, '/bin/opencode');
  assert.equal(clients.discovery.binaries.codex, '/bin/codex');
  assert.equal(clients.targets.codex.gatewayBaseUrl, 'http://127.0.0.1:4100');
  assert.equal(clients.gitRepoPath, '/main');
  assert.equal(clients.gitRepoRevision, 'c'.repeat(40));
  assert.equal(typeof clients.mockSnapshot, 'function');
  const fixture = calls.find((call) => Array.isArray(call) && call[0] === 'fixture:create');
  assert.equal(fixture[1].apiPort, 7800);
  assert.ok(calls.indexOf(probe) < calls.indexOf(fixture));
  assert.equal(calls.indexOf(fixture) < calls.findIndex((call) => Array.isArray(call) && call[0] === 'clients'), true);
  assert.deepEqual(cleanup, ['fixture', 'mock', 'database', 'tmux']);
});

test('WP-14A local client or reconciliation failure fails local acceptance', async () => {
  const { deps, calls, cleanup } = dependencies({ failAt: 'clients' });
  const result = await runLocalAcceptance({}, deps);
  assert.equal(result.status, 'fail');
  assert.equal(result.protocol.status, 'pass');
  assert.equal(result.clients.status, 'fail');
  assert.deepEqual(cleanup, ['fixture', 'mock', 'database', 'tmux']);
  assert.equal(calls.filter((call) => Array.isArray(call) && call[0] === 'fixture:create').length, 1);
});

test('AC-027 controlled negative: protocol failure still executes the complete owned cleanup stack', async () => {
  const { deps, cleanup } = dependencies({ failAt: 'protocol' });
  const result = await runLocalAcceptance({}, deps);
  assert.equal(result.status, 'fail');
  assert.equal(result.clients, null);
  assert.deepEqual(cleanup, ['fixture', 'mock', 'database', 'tmux']);
});

test('AC-028 controlled negatives: preflight and same-URL probe fail before runtime', async () => {
  for (const failAt of ['preflight', 'probe']) {
    const { deps, calls, cleanup } = dependencies({ failAt });
    const result = await runLocalAcceptance({}, deps);
    assert.equal(result.status, 'fail');
    assert.equal(calls.some((call) => Array.isArray(call) && call[0] === 'fixture:create'), false);
    assert.equal(calls.some((call) => Array.isArray(call) && call[0] === 'clients'), false);
    if (failAt === 'preflight') assert.deepEqual(cleanup, ['tmux']);
    else assert.deepEqual(cleanup, ['database', 'tmux']);
  }
});
