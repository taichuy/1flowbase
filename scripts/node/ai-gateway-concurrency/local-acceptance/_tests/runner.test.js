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
    database: { container: 'docker-db-1', image: 'postgres:16-alpine', host: '127.0.0.1', port: 35432 },
    artifacts: {
      apiServer: { path: '/bin/api-server', sha256: '1'.repeat(64) },
      pluginRunner: { path: '/bin/plugin-runner', sha256: '2'.repeat(64) },
      openaiPackage: { path: '/packages/openai', sha256: '3'.repeat(64) },
      anthropicPackage: { path: '/packages/anthropic', sha256: '4'.repeat(64) },
    },
  };
}

function dependencies({ failAt } = {}) {
  const calls = [];
  const cleanup = [];
  const deps = {
    loadManifest() { calls.push('manifest'); return fixtureManifest(); },
    async preflight() { calls.push('preflight'); if (failAt === 'preflight') throw new Error('preflight failed'); },
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
        async start() { calls.push('mock:start'); return { httpBaseUrl: 'http://127.0.0.1:4000' }; },
        snapshot() { return { active: 0, entries: [] }; },
        async stop() { cleanup.push('mock'); },
      };
    },
    async createGatewayFixture() {
      calls.push('fixture:create');
      if (failAt === 'fixture') throw new Error('fixture failed');
      return { result: { schema_version: '1flowbase.ai-gateway-fixture/v1' }, async close() { cleanup.push('fixture'); } };
    },
    writeReadyManifest() { calls.push('ready:write'); return '/evidence/ready.json'; },
    async runClientCompatibility(options) { calls.push(['smoke', options]); if (failAt === 'smoke') throw new Error('smoke failed'); return { status: 'pass', clients: {} }; },
    writeSnapshot() { calls.push('snapshot:write'); },
    writeResult(_root, result) { calls.push(['result', result.status]); },
  };
  return { deps, calls, cleanup };
}

test('AC-003/014/027/028/029: one attempt probes the exact URL then runs the portable ACP gate', async () => {
  const { deps, calls, cleanup } = dependencies();
  const result = await runLocalAcceptance({}, deps);
  assert.equal(result.status, 'pass', JSON.stringify(result));
  assert.equal(calls.filter((call) => call === 'database:create').length, 1);
  assert.equal(calls.filter((call) => call === 'fixture:create').length, 1);
  const probe = calls.find((call) => Array.isArray(call) && call[0] === 'database:probe');
  const smoke = calls.find((call) => Array.isArray(call) && call[0] === 'smoke')[1];
  assert.equal(probe[1], 'postgres://role:password@127.0.0.1:35432/database');
  assert.match(smoke.runtimeRoot, /client-compatibility\/runtime$/u);
  assert.ok(calls.indexOf(probe) < calls.indexOf('fixture:create'));
  assert.deepEqual(cleanup, ['fixture', 'mock', 'database']);
});

test('AC-027 controlled negative: every runtime failure still executes the complete owned cleanup stack', async () => {
  const { deps, calls, cleanup } = dependencies({ failAt: 'smoke' });
  const result = await runLocalAcceptance({}, deps);
  assert.equal(result.status, 'fail');
  assert.deepEqual(cleanup, ['fixture', 'mock', 'database']);
  assert.equal(calls.filter((call) => call === 'fixture:create').length, 1);
});

test('AC-028 controlled negatives: preflight and same-URL probe fail before runtime', async () => {
  for (const failAt of ['preflight', 'probe']) {
    const { deps, calls, cleanup } = dependencies({ failAt });
    const result = await runLocalAcceptance({}, deps);
    assert.equal(result.status, 'fail');
    assert.equal(calls.includes('fixture:create'), false);
    assert.equal(calls.some((call) => Array.isArray(call) && call[0] === 'smoke'), false);
    if (failAt === 'preflight') assert.deepEqual(cleanup, []);
    else assert.deepEqual(cleanup, ['database']);
  }
});
