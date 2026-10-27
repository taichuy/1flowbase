'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { cpuDelta, sampledPeaks, tree } = require('../proc.cjs');
const { configuration, publicConfiguration, connectionEnvironment, safeError } = require('../config.cjs');
const { apiSpawner } = require('../run.cjs');
const { spawnOwned } = require('../../gateway-fixture/process-owner');
const { psqlEnvironment } = require('../database.cjs');
const { outputOracle, requestBody, TEXT, DELTAS } = require('../payload.cjs');
const { wireEvents } = require('../mock.cjs');
const { assertNoArtifactSecrets } = require('../../cli-smoke/artifact-scan');
const row = (cpu, extra = {}) => ({ pid: 12, start_ticks: 123, cpu_ticks: cpu, group: 'api', pss_bytes: 100, ...extra });
const sample = (rows, complete = true) => ({ rows, complete, timestamp_ns: '1' });
function benchmarkEnvironment(extra = {}) {
  return { RB_REPO_ROOT: '/fixture', RB_ARTIFACT_ROOT: '/fixture/tmp/test-governance/resource-plan', RB_DEDICATED_DATABASE: '1',
    RB_DATABASE_URL: 'postgres://localhost/test_resource', RB_API_BINARY: '/binary', RB_OPENAI_PACKAGE: '/openai',
    RB_ANTHROPIC_PACKAGE: '/anthropic', RB_OPENAI_COMPATIBLE_PACKAGE: '/compatible', ...extra };
}
test('configuration preserves default and 1/8 dimensions, adds 3 and leaves unconfigured reports unchanged', () => {
  const defaults = configuration(benchmarkEnvironment());
  assert.equal(defaults.concurrency, 1);
  assert.equal(Object.hasOwn(defaults, 'connectionPlan'), false);
  assert.deepEqual(connectionEnvironment(defaults), {});
  assert.equal(Object.hasOwn(publicConfiguration(defaults), 'connectionPlan'), false);
  for (const concurrency of [1, 3, 8]) for (const bytes of [8192, 131072, 524288]) {
    const config = configuration(benchmarkEnvironment({ RB_CONCURRENCY: String(concurrency), RB_HISTORY_BYTES: String(bytes) }));
    assert.equal(config.concurrency, concurrency);
    assert.equal(config.historyBytes, bytes);
  }
  for (const concurrency of ['0', '2', '4', '7', '9']) assert.throws(() => configuration(benchmarkEnvironment({ RB_CONCURRENCY: concurrency })));
  for (const port of ['7600', '7800']) assert.throws(() => configuration(benchmarkEnvironment({ RB_API_PORT: port })), /protected API port/);
  assert.throws(() => configuration(benchmarkEnvironment({ RB_ARTIFACT_ROOT: '/outside/report' })), /artifact root/);
});
test('connection plan parses positive Rust u32 values and the existing known-allocation overflow rule', () => {
  const plans = [
    [{ RB_DATABASE_POOL_MAX_CONNECTIONS: '5' }, { API_DATABASE_POOL_MAX_CONNECTIONS: 5 }],
    [{ RB_DATABASE_POOL_MAX_CONNECTIONS: '10', RB_DATABASE_MANAGED_MAX_CONNECTIONS: '+0002' }, { API_DATABASE_POOL_MAX_CONNECTIONS: 10, API_DATABASE_MANAGED_MAX_CONNECTIONS: 2 }],
    [{ RB_DATABASE_POOL_MAX_CONNECTIONS: '2147483647' }, { API_DATABASE_POOL_MAX_CONNECTIONS: 2147483647 }],
    [{ RB_DATABASE_MANAGED_MAX_CONNECTIONS: '4294967290' }, { API_DATABASE_MANAGED_MAX_CONNECTIONS: 4294967290 }],
    [{ RB_DATABASE_POOL_MAX_CONNECTIONS: '4294967294', RB_DATABASE_MANAGED_MAX_CONNECTIONS: '1' }, { API_DATABASE_POOL_MAX_CONNECTIONS: 4294967294, API_DATABASE_MANAGED_MAX_CONNECTIONS: 1 }],
  ];
  for (const [env, expected] of plans) {
    const config = configuration(benchmarkEnvironment(env));
    assert.deepEqual(config.connectionPlan, expected);
    assert.deepEqual(publicConfiguration(config).connectionPlan, expected);
    assert.deepEqual(connectionEnvironment(config), Object.fromEntries(Object.entries(expected).map(([name, value]) => [name, String(value)])));
  }
  for (const name of ['RB_DATABASE_POOL_MAX_CONNECTIONS', 'RB_DATABASE_MANAGED_MAX_CONNECTIONS']) {
    for (const value of ['', ' ', ' 5', '5 ', '0', '-1', '1.5', '5.0', '5e0', '0x10', 'NaN', 'Infinity', '4294967296', true, null, 5]) {
      assert.throws(() => configuration(benchmarkEnvironment({ [name]: value })), new RegExp(name));
    }
  }
  for (const env of [
    { RB_DATABASE_POOL_MAX_CONNECTIONS: '2147483648' },
    { RB_DATABASE_MANAGED_MAX_CONNECTIONS: '4294967291' },
    { RB_DATABASE_POOL_MAX_CONNECTIONS: '4294967295', RB_DATABASE_MANAGED_MAX_CONNECTIONS: '1' },
  ]) assert.throws(() => configuration(benchmarkEnvironment(env)), /connection budget overflow/);
  const unsupported = configuration(benchmarkEnvironment({ RB_DATABASE_INSTANCE_MAX_CONNECTIONS: '100', RB_DATABASE_DISPATCH_MAX_CONNECTIONS: '20' }));
  assert.deepEqual(connectionEnvironment(unsupported), {}, 'unsupported variables must not become generic env forwarding');
});
test('report and spawn plan whitelist reject nonnumeric known fields without exposing their contents', () => {
  const canary = 'private-plan-value-canary';
  for (const value of [canary, true, null, {}, 0, 1.5, Infinity, NaN, 4294967296]) {
    const config = { connectionPlan: { API_DATABASE_POOL_MAX_CONNECTIONS: value } };
    for (const operation of [publicConfiguration, connectionEnvironment]) {
      assert.throws(() => operation(config), error => error.message === 'invalid API_DATABASE_POOL_MAX_CONNECTIONS' && !error.message.includes(canary));
    }
  }
  for (const plan of [null, [], canary]) assert.throws(() => publicConfiguration({ connectionPlan: plan }), /invalid connection plan/);
});
test('production API spawner applies P/M after fixture defaults and preserves unconfigured 1/8 behavior', () => {
  for (const concurrency of [1, 3, 8]) for (const planned of [false, true]) {
    const config = configuration(benchmarkEnvironment({ RB_CONCURRENCY: String(concurrency),
      ...(planned ? { RB_DATABASE_POOL_MAX_CONNECTIONS: '10', RB_DATABASE_MANAGED_MAX_CONNECTIONS: '2' } : {}) }));
    if (planned) Object.assign(config.connectionPlan, { API_DATABASE_URL: 'private-plan-url', API_DATABASE_INSTANCE_MAX_CONNECTIONS: 99, NODE_OPTIONS: 'private-plan-options' });
    let actual;
    const child = { stdout: null, stderr: null };
    const start = apiSpawner(config, (binary, env, options) => spawnOwned(binary, env, options, (binary, args, options) => {
      actual = { binary, args, ...options };
      return child;
    }));
    const owned = start('/binary', { API_DATABASE_POOL_MAX_CONNECTIONS: '5', API_DATABASE_URL: 'postgres://localhost/test_fixture', API_DATABASE_DISPATCH_MAX_CONNECTIONS: '11' },
      { cwd: '/owned-scratch', parentEnv: { API_DATABASE_POOL_MAX_CONNECTIONS: '1', API_DATABASE_MANAGED_MAX_CONNECTIONS: '9', PRIVATE_PARENT: 'private-parent-canary' } });
    assert.equal(owned.child, child);
    assert.equal(actual.env.API_DATABASE_POOL_MAX_CONNECTIONS, planned ? '10' : '5');
    assert.equal(actual.env.API_DATABASE_MANAGED_MAX_CONNECTIONS, planned ? '2' : '9');
    assert.equal(actual.env.API_DATABASE_URL, 'postgres://localhost/test_fixture');
    assert.equal(actual.env.API_DATABASE_DISPATCH_MAX_CONNECTIONS, '11');
    assert.equal(actual.env.API_DATABASE_INSTANCE_MAX_CONNECTIONS, undefined);
    assert.equal(actual.env.API_DATABASE_RESERVED_CONNECTIONS, undefined);
    assert.equal(actual.env.NODE_OPTIONS, undefined);
    assert.equal(actual.cwd, '/fixture/api');
    assert.equal(actual.env.API_MCP_TEMPLATE_LIBRARY_ROOT, '/owned-scratch/mcp-library');
    assert.equal(actual.env.API_APPLICATION_TEMPLATE_ROOT, '/fixture/api/resources/application-templates');
    assert.deepEqual(actual.args, []);
    assert.ok(!JSON.stringify(publicConfiguration(config)).includes('private-'));
  }
});
test('CPU conversion uses provided CLK_TCK and rejects reused PID, regression and incomplete enumeration', () => {
  assert.equal(cpuDelta(sample([row(20)]), sample([row(60)]), 250).api.cpu_seconds, 0.16);
  for (const after of [sample([row(60, { start_ticks: 124 })]), sample([row(10)]), sample([row(60)] , false), sample([])]) {
    assert.equal(cpuDelta(sample([row(20)]), after, 250).api.cpu_seconds, null);
  }
  assert.equal(cpuDelta(sample([row(20)]), sample([row(60)]), 250).database.cpu_seconds, null);
});
test('enumeration includes children of every thread and exposes unreadable children', () => {
  const files = { '/proc/1/task/a/children': '2', '/proc/1/task/b/children': '3', '/proc/2/task/a/children': '', '/proc/3/task/a/children': '' };
  const list = p => p === '/proc/1/task' ? ['a', 'b'] : ['a'];
  assert.deepEqual(tree(1, p => files[p], list), { pids: [1, 2, 3], complete: true });
  const incomplete = tree(1, p => { if (p.includes('/b/')) throw Error('gone'); return files[p]; }, list);
  assert.equal(incomplete.complete, false);
  assert.deepEqual(incomplete.pids, [1, 2]);
});
test('sampled PSS sums co-observed rows before taking peak and never treats missing data as zero', () => {
  const first = sample([row(1, { group: 'plugin', pss_bytes: 100 }), row(1, { pid: 13, group: 'plugin', pss_bytes: 10 })]);
  const second = sample([row(1, { group: 'plugin', pss_bytes: 10 }), row(1, { pid: 13, group: 'plugin', pss_bytes: 100 })]);
  const third = sample([row(1, { group: 'plugin', pss_bytes: null })]);
  const peaks = sampledPeaks([first, second, third]);
  assert.equal(peaks.plugin.pss_bytes, 110);
  assert.equal(peaks.plugin.pss_bytes_missing_samples, 1);
  assert.equal(peaks.database.pss_bytes, null);
});
test('group-specific completeness preserves API but invalidates missing DB/plugin data', () => {
  const before = { ...sample([row(20), row(30, { pid: 30, group: 'database' })]), group_complete: { api: true, plugin: false, database: false } };
  const after = { ...sample([row(60), row(90, { pid: 30, group: 'database' })]), group_complete: { api: true, plugin: false, database: false } };
  assert.equal(cpuDelta(before, after, 100).api.cpu_seconds, 0.4);
  assert.equal(cpuDelta(before, after, 100).database.cpu_seconds, null);
  assert.equal(cpuDelta(before, after, 100).plugin.cpu_seconds, null);
  const memory = sampledPeaks([sample([row(1, { rss_bytes: 200, pss_bytes: 100 })])]);
  assert.equal(memory.api.rss_bytes, 200);
  assert.equal(memory.api.pss_bytes, 100);
});
test('psql credentials are env-only and sanitized errors keep diagnostic context', () => {
  const url = 'postgres://observer:credential-canary@127.0.0.1:1234/qadb_test';
  const env = psqlEnvironment(url);
  assert.equal(env.PGPASSWORD, 'credential-canary');
  assert.equal(env.PGDATABASE, 'qadb_test');
  const error = safeError(Object.assign(new Error('connection failed credential-canary ' + url), { code: 'ECONNREFUSED' }), [url, 'credential-canary']);
  assert.equal(error.code, 'ECONNREFUSED');
  assert.ok(error.message.includes('connection failed'));
  assert.ok(!JSON.stringify(error).includes('credential-canary'));
});
test('oracle rejects truncated streams, duplicate completion, wrong text and error terminal', () => {
  const events = wireEvents();
  assert.equal(outputOracle(events).delta_count, DELTAS);
  assert.throws(() => outputOracle(events.slice(0, -1)));
  assert.throws(() => outputOracle([...events, events.at(-1)]));
  assert.throws(() => outputOracle([...events, { type: 'error' }]));
  assert.throws(() => outputOracle(events, TEXT));
  const partialTerminal = structuredClone(events);
  partialTerminal.at(-1).response.output[0].content[0].text = TEXT;
  assert.throws(() => outputOracle(partialTerminal));
  assert.throws(() => outputOracle(events.filter((e, i) => i !== events.findIndex(x => x.type === 'response.output_text.delta'))));
});
test('history uses Responses message shape with exact UTF-8 text budget', () => {
  for (const bytes of [8192, 131072, 524288]) {
    const payload = JSON.parse(requestBody(bytes, '1flowbase'));
    assert.equal(payload.input.reduce((n, e) => n + Buffer.byteLength(e.content[0].text), 0), bytes);
    assert.equal(payload.input.length, 128);
    assert.ok(payload.input.every(e => e.role === 'user' && e.content[0].type === 'input_text'));
  }
});
test('artifact configuration excludes secret env and scanner rejects deliberate canary', () => {
  const canary = 'private-database-password-canary';
  const published = publicConfiguration({ historyBytes: 8192, databaseUrl: `postgres://root:${canary}@localhost/tmp_test`, env: { TOKEN: canary }, cookie: canary,
    ownerSessions: [canary], fixture: { result: { api_key: canary } }, connectionPlan: {
      API_DATABASE_POOL_MAX_CONNECTIONS: 10, API_DATABASE_MANAGED_MAX_CONNECTIONS: 2,
      API_DATABASE_URL: canary, OWNER_SESSION: canary, API_DATABASE_INSTANCE_MAX_CONNECTIONS: 99,
    } });
  assert.deepEqual(published.connectionPlan, { API_DATABASE_POOL_MAX_CONNECTIONS: 10, API_DATABASE_MANAGED_MAX_CONNECTIONS: 2 });
  const serialized = JSON.stringify(published);
  assert.ok(!serialized.includes(canary));
  assert.ok(!serialized.includes('databaseUrl'));
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'resource-artifact-test-'));
  try {
    fs.writeFileSync(path.join(root, 'report.json'), serialized);
    assert.equal(assertNoArtifactSecrets([root], [canary]).length, 1);
    fs.writeFileSync(path.join(root, 'leak.txt'), canary);
    assert.throws(() => assertNoArtifactSecrets([root], [canary]), /secret canary/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
