'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { cpuDelta, sampledPeaks, tree } = require('../proc.cjs');
const { publicConfiguration, safeError } = require('../config.cjs');
const { psqlEnvironment } = require('../database.cjs');
const { outputOracle, requestBody, TEXT, DELTAS } = require('../payload.cjs');
const { wireEvents } = require('../mock.cjs');
const { assertNoArtifactSecrets } = require('../../cli-smoke/artifact-scan');
const row = (cpu, extra = {}) => ({ pid: 12, start_ticks: 123, cpu_ticks: cpu, group: 'api', pss_bytes: 100, ...extra });
const sample = (rows, complete = true) => ({ rows, complete, timestamp_ns: '1' });
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
  const serialized = JSON.stringify(publicConfiguration({ historyBytes: 8192, databaseUrl: `postgres://root:${canary}@localhost/tmp_test`, env: { TOKEN: canary }, cookie: canary }));
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
