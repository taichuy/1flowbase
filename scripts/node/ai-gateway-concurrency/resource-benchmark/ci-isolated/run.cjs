'use strict';
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const cp = require('node:child_process');
const { once } = require('node:events');
const { delay } = require('../mock.cjs');
const { psqlEnvironment } = require('../database.cjs');
const { redactServiceLog } = require('../../gateway-fixture/service-logs');
const { assertNoArtifactSecrets } = require('../../cli-smoke/artifact-scan');
const { startMonitor } = require('./light-monitor.cjs');
const manifest = require('./manifest.json');
const root = path.resolve(__dirname, '../../../../..');
const out = path.join(root, 'tmp/test-governance/gateway-independent-resource');
const privateRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'gateway-independent-'));
const write = (file, value) => fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n', { mode: 0o600 });
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
function execute(binary, args, options = {}) {
  return cp.execFileSync(binary, args, { encoding: 'utf8', timeout: 30000, maxBuffer: 16 * 1024 * 1024, ...options }).trim();
}
const docker = args => execute('docker', args);
const owned = new Map();
function ownerReceipt() { write(path.join(out, 'owned-containers.json'), [...owned.values()].map(({ id, name, token, pid, startTicks, volumeIds }) => ({ id, name, token, pid, startTicks, volumeIds }))); }
const passwords = [];
let activeChild = null, interrupted = false;
function interrupt() { interrupted = true; activeChild?.kill('SIGINT'); }
process.on('SIGINT', interrupt);
process.on('SIGTERM', interrupt);
function query(url, sql) {
  return execute('/usr/bin/psql', ['-X', '-A', '-t', '-w', '-v', 'ON_ERROR_STOP=1', '-c', sql], { env: psqlEnvironment(url) });
}
function statIdentity(pid) {
  const raw = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
  return raw.slice(raw.lastIndexOf(')') + 2).trim().split(/\s+/)[19];
}
function inspect(id) { return JSON.parse(docker(['inspect', id]))[0]; }
function anonymousVolumes(actual) {
  const ids = actual.Mounts.filter(mount => mount.Type === 'volume').map(mount => mount.Name).sort();
  if (ids.some(id => !/^[a-f0-9]{64}$/.test(id))) throw Error('unexpected non-anonymous fixture volume');
  return ids;
}
function verifyVolumesRemoved(ids) {
  for (const id of ids) {
    if (!/^[a-f0-9]{64}$/.test(id)) throw Error('invalid owned volume identity');
    if (docker(['volume', 'ls', '--format', '{{.Name}}', '--filter', `name=^${id}$`])) throw Error('owned anonymous volume cleanup incomplete');
  }
}
function guard(owner, running = true) {
  const actual = inspect(owner.id);
  if (actual.Id !== owner.id || actual.Name !== '/' + owner.name || actual.Config.Labels['1flowbase.fixture-owner'] !== owner.token) throw Error('container ownership mismatch');
  if (running && (!actual.State.Running || actual.State.Pid !== owner.pid || statIdentity(owner.pid) !== owner.startTicks)) throw Error('container PID identity mismatch');
  return actual;
}
function remove(owner) {
  const actual = guard(owner, false);
  if (actual.State.Running) guard(owner);
  const volumeIds = anonymousVolumes(actual);
  if (owner.volumeIds && JSON.stringify(owner.volumeIds) !== JSON.stringify(volumeIds)) throw Error('container volume identity mismatch');
  owner.volumeIds = volumeIds; ownerReceipt();
  docker(['rm', '-f', '-v', owner.id]);
  // Exact ID only; never broad docker pruning or name-pattern deletion.
  const remains = docker(['ps', '-aq', '--no-trunc', '--filter', `id=${owner.id}`]);
  if (remains) throw Error('owned container cleanup incomplete');
  verifyVolumesRemoved(volumeIds);
  owned.delete(owner.id); ownerReceipt();
  return { id: owner.id, cleanup: 'complete', anonymous_volume_ids: volumeIds, removed_volume_ids: volumeIds, volume_cleanup: 'verified_absent' };
}
async function createDatabase(label) {
  const token = crypto.randomUUID();
  const name = 'gateway-independent-' + token;
  const password = crypto.randomBytes(32).toString('hex');
  passwords.push(password);
  const envFile = path.join(privateRoot, label + '.env');
  fs.writeFileSync(envFile, `POSTGRES_PASSWORD=${password}\nPOSTGRES_USER=postgres\nPOSTGRES_DB=gateway_fixture\n`, { mode: 0o600 });
  const id = docker(['run', '--pull=never', '-d', '--name', name, '--label', `1flowbase.fixture-owner=${token}`, '--env-file', envFile,
    '-p', '127.0.0.1::5432', manifest.postgres.image, '-c', 'shared_preload_libraries=pg_stat_statements',
    '-c', 'pg_stat_statements.track=all', '-c', 'pg_stat_statements.track_planning=on']);
  const owner = { id, name, token, envFile };
  owned.set(id, owner); ownerReceipt();
  const actual = inspect(id);
  owner.volumeIds = anonymousVolumes(actual); ownerReceipt();
  owner.pid = actual.State.Pid;
  owner.startTicks = statIdentity(owner.pid); ownerReceipt();
  const port = actual.NetworkSettings.Ports['5432/tcp'][0];
  if (port.HostIp !== '127.0.0.1' || [7600, 7800].includes(Number(port.HostPort))) throw Error('unsafe postgres port');
  owner.url = `postgres://postgres:${password}@127.0.0.1:${port.HostPort}/gateway_fixture`;
  // Resolve the actual cgroup v2 path from the owned container PID, portable across runner Docker drivers.
  const group = fs.readFileSync(`/proc/${owner.pid}/cgroup`, 'utf8').split('\n').find(line => line.startsWith('0::'));
  if (!group) throw Error('cgroup v2 unavailable');
  owner.cgroup = path.join('/sys/fs/cgroup', group.slice(3));
  fs.readFileSync(path.join(owner.cgroup, 'cpu.stat'), 'utf8');
  let ready = false;
  for (let i = 0; i < 120 && !interrupted; i++) {
    guard(owner);
    try { query(owner.url, 'SELECT 1'); ready = true; break; } catch { await delay(500); }
  }
  if (!ready) throw Error('owned postgres not ready');
  query(owner.url, 'CREATE EXTENSION pg_stat_statements');
  owner.version = query(owner.url, 'SELECT version()');
  owner.image = actual.Image;
  return owner;
}
function sqlStats(owner) {
  guard(owner);
  const raw = query(owner.url, "SELECT coalesce(json_agg(t),'[]') FROM (SELECT s.*,s.queryid::text AS queryid FROM pg_stat_statements s WHERE dbid=(SELECT oid FROM pg_database WHERE datname=current_database()) ORDER BY s.queryid,userid,toplevel) t");
  // Retain every statement and statistic; SQL text is normalized by PG, with credential scrubber as a second boundary.
  return JSON.parse(redactServiceLog(raw, passwords));
}
function captureIntegrity(owner) {
  return JSON.parse(query(owner.url, `SELECT json_build_object(
    'captures',count(*),'complete',count(*) FILTER(WHERE status='complete'),
    'dropped',coalesce(sum(dropped_count),0),'persist_failed',coalesce(sum(persist_failed_count),0),
    'steps',(SELECT count(*) FROM client_trajectory_steps),'sections',(SELECT count(*) FROM client_trajectory_sections),
    'events',(SELECT count(*) FROM runtime_events),'high_water',(SELECT sum(runtime_event_sequence_high_water) FROM flow_runs),
    'raw_frames',(SELECT sum(persisted_through) FROM client_trajectory_archive_heads)) FROM client_trajectory_captures`));
}
function pgSample(owner) {
  return { timestamp_ns: String(process.hrtime.bigint()), cpu: Object.fromEntries(fs.readFileSync(path.join(owner.cgroup, 'cpu.stat'), 'utf8').trim().split('\n').map(line => {
    const [key, value] = line.split(/\s+/); return [key, Number(value)];
  })) };
}
function boundaryCpu(samples, before, after) {
  const start = BigInt(before.timestamp_ns), end = BigInt(after.timestamp_ns);
  const first = [...samples].reverse().find(s => BigInt(s.timestamp_ns) <= start);
  const last = samples.find(s => BigInt(s.timestamp_ns) >= end);
  if (!first || !last || !Number.isFinite(first.cpu.usage_usec) || !Number.isFinite(last.cpu.usage_usec)) return null;
  return { cpu_seconds: (last.cpu.usage_usec - first.cpu.usage_usec) / 1e6,
    start_alignment_ms: Number(start - BigInt(first.timestamp_ns)) / 1e6,
    end_alignment_ms: Number(BigInt(last.timestamp_ns) - end) / 1e6 };
}
function sqlDeltas(before, after) {
  return after.map(now => {
    const old = before.find(row => row.queryid === now.queryid && row.userid === now.userid && row.toplevel === now.toplevel);
    return { queryid: now.queryid, userid: now.userid, toplevel: now.toplevel, query: now.query,
      ...Object.fromEntries(['calls', 'plans', 'total_plan_time', 'total_exec_time', 'rows'].map(k => [k, Number(now[k]) - Number(old?.[k] || 0)])) };
  });
}
function summarize(report, samples, before, after) {
  const latencies = report.rounds.flatMap(round => round.workload.value.requests.map(request => request.elapsed_ms)).sort((a,b) => a-b);
  const windows = report.rounds.map(round => {
    const window = phase => {
      const measurement = round[phase];
      const pg = boundaryCpu(samples, measurement.samples[0], measurement.samples.at(-1));
      const api = measurement.cpu.api.cpu_seconds, plugin = measurement.cpu.plugin.cpu_seconds;
      if (api === null || plugin === null || !pg || pg.cpu_seconds < 0) throw Error('required CPU evidence unavailable');
      return { api_cpu_seconds: api, plugin_cpu_seconds: plugin, pg_cgroup: pg,
        cpu_seconds_per_request: { api: api / report.configuration.concurrency, plugin: plugin / report.configuration.concurrency, pg: pg.cpu_seconds / report.configuration.concurrency } };
    };
    return { ordinal: round.ordinal, request: window('workload'), drain: window('drain') };
  });
  const totals = { api: 0, plugin: 0, pg: 0 };
  for (const round of windows) for (const phase of ['request', 'drain']) {
    totals.api += round[phase].api_cpu_seconds; totals.plugin += round[phase].plugin_cpu_seconds; totals.pg += round[phase].pg_cgroup.cpu_seconds;
  }
  return { windows, request_plus_drain_cpu_seconds: totals, request_plus_drain_cpu_seconds_per_request: Object.fromEntries(Object.entries(totals).map(([key, value]) => [key, value / (6 * report.configuration.concurrency)])), p95_latency_ms: latencies[Math.ceil(latencies.length * 0.95)-1], query_deltas: sqlDeltas(before, after),
    cpu_ticks_per_second: report.clock_ticks_per_second, limitations: manifest.limitations };
}
async function prepareInputs() {
  const inputs = { binaries: {}, plugins: {} };
  const inputRoot = path.join(root, 'tmp/ci-independent-inputs');
  fs.mkdirSync(inputRoot, { recursive: true });
  for (const [label, expected] of Object.entries(manifest.binaries)) {
    const dir = path.join(inputRoot, label);
    for (const [file, value] of [['source-sha.txt', expected.source], ['api-tree.txt', expected.api_tree], ['rustflags.txt', manifest.build.rustflags], ['build-exit-code.txt', '0'], ['source-status.txt', '']]) {
      if (fs.readFileSync(path.join(dir, file), 'utf8').trim() !== value) throw Error('artifact provenance mismatch: ' + label + '/' + file);
    }
    if (!fs.readFileSync(path.join(dir, 'toolchain.txt'), 'utf8').includes('rustc ' + manifest.build.rust)) throw Error('artifact toolchain mismatch');
    const archive = path.join(dir, 'api-server.tar.gz');
    const members = execute('tar', ['-tzf', archive]).split('\n');
    if (members.length !== 1 || members[0] !== 'api-server') throw Error('unexpected binary archive layout');
    execute('tar', ['-xzf', archive, '-C', dir]);
    const binary = path.join(dir, 'api-server');
    if (hash(binary) !== expected.sha256 || fs.readFileSync(path.join(dir, 'binary.sha256'), 'utf8').trim().split(/\s+/)[0] !== expected.sha256) throw Error('artifact binary digest mismatch');
    fs.chmodSync(binary, 0o700);
    inputs.binaries[label] = binary;
  }
  for (const [label, expected] of Object.entries(manifest.plugins)) {
    const file = expected.path ? path.join(__dirname, expected.path) : path.join(inputRoot, label + '.1flowbasepkg');
    if (expected.url) execute('curl', ['--fail', '--location', '--silent', '--show-error', '--retry', '3', '--max-time', '180', '--output', file, expected.url], { timeout: 600000 });
    if (hash(file) !== expected.sha256 || (expected.bytes && fs.statSync(file).size !== expected.bytes)) throw Error('plugin digest mismatch: ' + label);
    inputs.plugins[label] = file;
  }
  return inputs;
}
async function cell(label, variant, dimension, inputs) {
  const dir = path.join(out, label);
  fs.mkdirSync(dir, { recursive: true });
  let owner, stopLight, timer, failure = null, childResult = null;
  let ready = null, before = null, after = null;
  const pgSamples = [], cost = { user_us: 0, system_us: 0, wall_ms: 0, samples: 0 };
  const logs = [];
  try {
    if (interrupted) throw Error('fixture interrupted');
    owner = await createDatabase(label);
    write(path.join(dir, 'provenance.json'), { fixture_sha: execute('git', ['rev-parse', 'HEAD']), binary: manifest.binaries[variant], plugins: manifest.plugins,
      container: { id: owner.id, name: owner.name, pid: owner.pid, start_ticks: owner.startTicks, image_id: owner.image, postgres_version: owner.version },
      planning_telemetry: manifest.postgres, dimension });
    const sample = () => {
      const begin = performance.now(), usage = process.cpuUsage();
      try {
        pgSamples.push(pgSample(owner));
        if (!ready && fs.existsSync(path.join(dir, 'ready.json'))) {
          ready = JSON.parse(fs.readFileSync(path.join(dir, 'ready.json')));
          if ([7600, 7800].includes(ready.loopback_port) || ready.api_binary_sha256 !== manifest.binaries[variant].sha256 || statIdentity(ready.gateway_pid) !== String(ready.start_ticks)) throw Error('API ready ownership mismatch');
          stopLight = startMonitor(dir, ready.gateway_pid, inputs.binaries[variant]);
          before = sqlStats(owner);
          write(path.join(dir, 'sql-before.json'), before);
        }
      } catch (error) { failure ||= error; activeChild?.kill('SIGINT'); }
      finally { const used = process.cpuUsage(usage); cost.user_us += used.user; cost.system_us += used.system; cost.wall_ms += performance.now()-begin; cost.samples++; }
    };
    sample(); if (failure) throw failure; timer = setInterval(sample, 200);
    const child = cp.spawn(process.execPath, [path.join(__dirname, 'entry.cjs')], {
      cwd: root, stdio: ['ignore', 'pipe', 'pipe'], env: { PATH: process.env.PATH, HOME: privateRoot, LANG: 'C.UTF-8',
        API_PLUGIN_UPLOAD_MAX_BYTES: '67108864', RB_REPO_ROOT: root, RB_ARTIFACT_ROOT: dir,
        RB_API_BINARY: inputs.binaries[variant], RB_DATABASE_URL: owner.url, RB_DEDICATED_DATABASE: '1', RB_DEDICATED_CLUSTER: '1',
        RB_OPENAI_PACKAGE: inputs.plugins.openai, RB_ANTHROPIC_PACKAGE: inputs.plugins.anthropic, RB_OPENAI_COMPATIBLE_PACKAGE: inputs.plugins.openai_compatible,
        RB_HISTORY_BYTES: String(dimension.history_bytes), RB_CONCURRENCY: String(dimension.concurrency), RB_ROUNDS: '6', RB_WARMUPS: '1', RB_IDLE_MS: '10000', RB_DRAIN_MS: '5000' },
    });
    activeChild = child;
    for (const stream of [child.stdout, child.stderr]) stream.on('data', chunk => { logs.push(chunk); while (logs.reduce((sum,b) => sum+b.length,0)>65536) logs.shift(); });
    let forceTimer;
    const watchdog = setTimeout(() => { failure ||= Error('cell timeout'); child.kill('SIGINT'); forceTimer = setTimeout(() => child.kill('SIGKILL'), 35000); }, 180000);
    let result;
    try { result = await once(child, 'exit'); } finally { clearTimeout(watchdog); clearTimeout(forceTimer); }
    activeChild = null;
    childResult = { code: result[0], signal: result[1] };
    clearInterval(timer); timer = null; sample();
    stopLight?.(); stopLight = null;
    after = sqlStats(owner);
    write(path.join(dir, 'sql-after.json'), after);
    const integrity = captureIntegrity(owner);
    write(path.join(dir, 'capture-integrity.json'), integrity);
    if (integrity.captures !== 7 * dimension.concurrency || integrity.complete !== integrity.captures || integrity.dropped !== 0 || integrity.persist_failed !== 0) throw Error('capture integrity failed');
    if (failure) throw failure;
    if (result[0] !== 0 || result[1] || !ready || !before) throw Error('missing workload evidence');
    const report = JSON.parse(fs.readFileSync(path.join(dir, 'report.json')));
    if (report.outcome !== 'observations_collected' || report.cleanup !== 'complete' || report.rounds.length !== 6 || report.warmup.length !== 1) throw Error('benchmark oracle or cleanup failure');
    if (report.provenance.api_binary_sha256 !== manifest.binaries[variant].sha256 || Object.entries(manifest.plugins).some(([key,value]) => report.provenance.packages_sha256[key] !== value.sha256)) throw Error('report provenance mismatch');
    const summary = summarize(report, pgSamples, before, after);
    const light = JSON.parse(fs.readFileSync(path.join(dir, 'light-resources.json')));
    const pss = light.samples.filter(sample => Number.isFinite(sample.pss_bytes));
    if (light.error || !light.samples.length || !pss.length) throw Error('required light resource evidence unavailable');
    summary.api_light_monitor = { stat_period_ms: light.period_ms, pss_period_ms: light.pss_period_ms,
      sample_count: light.samples.length, pss_samples: pss.map(({ timestamp_ns, pss_bytes }) => ({ timestamp_ns, pss_bytes })),
      sampled_pss_peak_bytes: Math.max(...pss.map(sample => sample.pss_bytes)), observer_cost: light.cost };
    summary.pg_observer_cost = cost;
    write(path.join(dir, 'summary.json'), summary);
    return { label, variant, dimension, outcome: 'collected', summary };
  } catch (error) {
    failure ||= error;
    // Evidence stays on failure; continue subsequent ABBA cells instead of performance gating.
    return { label, variant, dimension, outcome: 'failed', error: redactServiceLog(error.message, passwords) };
  } finally {
    clearInterval(timer);
    if (activeChild) { activeChild.kill('SIGINT'); await Promise.race([once(activeChild, 'exit').catch(() => {}), delay(10000)]); if (activeChild.exitCode === null) activeChild.kill('SIGKILL'); activeChild = null; }
    try { stopLight?.(); } catch (error) { failure ||= error; }
    write(path.join(dir, 'database-cgroup.json'), { period_ms: 200, cost, samples: pgSamples });
    fs.writeFileSync(path.join(dir, 'driver.log'), redactServiceLog(Buffer.concat(logs).toString('utf8'), passwords), { mode: 0o600 });
    let cleanup = 'complete', databaseCleanup = null;
    if (ready) {
      try {
        // The existing fixture normally stops this process. Recover only the guarded owned API after an interrupted entry.
        const pid = ready.gateway_pid;
        const sameApi = () => fs.existsSync(`/proc/${pid}/stat`) && statIdentity(pid) === String(ready.start_ticks);
        if (sameApi()) {
          if (fs.realpathSync(`/proc/${pid}/exe`) !== fs.realpathSync(inputs.binaries[variant])) throw Error('API cleanup binary mismatch');
          process.kill(pid, 'SIGTERM');
          for (let i = 0; i < 30 && sameApi(); i++) await delay(100);
          if (sameApi()) { process.kill(pid, 'SIGKILL'); await delay(100); }
          if (sameApi()) throw Error('owned API cleanup incomplete');
        }
      } catch (error) { cleanup = 'failed'; failure ||= error; }
    }
    if (owner) {
      try { databaseCleanup = remove(owner); } catch (error) { cleanup = 'failed'; failure ||= error; databaseCleanup = { id: owner.id, cleanup: 'failed', anonymous_volume_ids: owner.volumeIds ?? [], volume_cleanup: 'unverified' }; }
      fs.rmSync(owner.envFile, { force: true });
    }
    write(path.join(dir, 'cleanup.json'), { cleanup, database: databaseCleanup, child: childResult, error: failure ? redactServiceLog(failure.message, passwords) : null });
    if (cleanup !== 'complete' || failure) process.exitCode = 1;
  }
}
async function main() {
  if (fs.existsSync(out)) throw Error('fresh artifact root required');
  fs.mkdirSync(out, { recursive: true });
  write(path.join(out, 'manifest.json'), manifest);
  write(path.join(out, 'environment.json'), { uname: execute('uname', ['-a']), lscpu: execute('lscpu', []), node: process.version,
    docker: execute('docker', ['version', '--format', '{{json .}}']), psql: execute('/usr/bin/psql', ['--version']), fixture_sha: execute('git', ['rev-parse', 'HEAD']) });
  const inputs = await prepareInputs();
  const cells = [];
  for (const [dimensionIndex, dimension] of manifest.dimensions.entries()) {
    for (const [orderIndex, variant] of manifest.order.entries()) {
      const label = `d${dimensionIndex+1}-${orderIndex+1}-${variant}`;
      cells.push(await cell(label, variant, dimension, inputs));
      if (interrupted) break;
    }
    if (interrupted) break;
  }
  const pairs = manifest.dimensions.map((dimension, index) => ({ dimension,
    ordered_pairs: [[0,1],[3,2]].map(([a,b]) => ({ baseline: cells[index*4+a]?.label, candidate: cells[index*4+b]?.label,
      baseline_cpu_per_request: cells[index*4+a]?.summary?.request_plus_drain_cpu_seconds_per_request ?? null,
      candidate_cpu_per_request: cells[index*4+b]?.summary?.request_plus_drain_cpu_seconds_per_request ?? null })) }));
  write(path.join(out, 'summary.json'), { cells, pairs, performance_gate: false, limitations: manifest.limitations });
  assertNoArtifactSecrets([out], passwords);
  if (cells.length !== 12 || cells.some(result => result.outcome !== 'collected')) process.exitCode = 1;
}
main().catch(error => {
  fs.mkdirSync(out, { recursive: true });
  write(path.join(out, 'failure.json'), { error: redactServiceLog(error.message, passwords) });
  process.exitCode = 1;
}).finally(() => {
  const cleanup = [];
  for (const owner of owned.values()) {
    try { cleanup.push(remove(owner)); }
    catch (error) { cleanup.push({ id: owner.id, cleanup: 'failed', anonymous_volume_ids: owner.volumeIds ?? [], volume_cleanup: 'unverified', error: redactServiceLog(error.message, passwords) }); process.exitCode = 1; }
  }
  write(path.join(out, 'final-cleanup.json'), cleanup);
  // Upload a strict sanitized whitelist, never scratch, env files, binary inputs or raw auth responses.
  try {
    assertNoArtifactSecrets([out], passwords);
    const published = path.join(root, 'tmp/test-governance/gateway-independent-resource-published');
    fs.mkdirSync(published, { recursive: true });
    const allowed = new Set(['manifest.json', 'environment.json', 'summary.json', 'failure.json', 'final-cleanup.json',
      'report.json', 'light-resources.json', 'database-cgroup.json', 'sql-before.json', 'sql-after.json',
      'capture-integrity.json', 'cleanup.json', 'provenance.json', 'service-api-server.log', 'driver.log']);
    function publish(dir) {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const source = path.join(dir, entry.name);
        if (entry.isDirectory()) publish(source);
        else if (entry.isFile() && allowed.has(entry.name)) {
          const target = path.join(published, path.relative(out, source));
          fs.mkdirSync(path.dirname(target), { recursive: true }); fs.copyFileSync(source, target);
        }
      }
    }
    publish(out);
  } catch { process.exitCode = 1; }
  fs.rmSync(privateRoot, { recursive: true, force: true });
  process.removeListener('SIGINT', interrupt); process.removeListener('SIGTERM', interrupt);
});
