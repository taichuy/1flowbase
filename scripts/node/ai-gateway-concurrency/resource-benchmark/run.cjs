#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { configuration, publicConfiguration, secrets, safeError } = require('./config.cjs');
const { clockTicks, parseStat } = require('./proc.cjs');
const { requestBody } = require('./payload.cjs');
const { startMock, delay } = require('./mock.cjs');
const { measure, request } = require('./measure.cjs');
const { databaseSnapshot } = require('./database.cjs');
const { assertNoArtifactSecrets } = require('../cli-smoke/artifact-scan');

async function run(config) {
  const { createGatewayFixture, sha256File } = require(path.join(config.repoRoot, 'scripts/node/ai-gateway-concurrency/gateway-fixture'));
  const { OwnerHttpClient } = require(path.join(config.repoRoot, 'scripts/node/ai-gateway-concurrency/gateway-fixture/http-owner'));
  const { reserveLoopbackPort } = require(path.join(config.repoRoot, 'scripts/node/ai-gateway-concurrency/gateway-fixture/process-owner'));
  const { openTemporaryOwnerSession } = require(path.join(config.repoRoot, 'scripts/node/page-debug/auth'));
  const sessions = [];
  const canaries = secrets(config);
  class TemporaryOwner extends OwnerHttpClient {
    async signIn(account, password) {
      const session = await openTemporaryOwnerSession({ apiBaseUrl: this.baseUrl, account, password });
      sessions.push(session);
      canaries.push(password, session.cookie, session.csrfToken);
      this.attachSession(session.cookie, session.csrfToken);
    }
  }
  const report = {
    schema_version: '1flowbase.gateway-resource-benchmark/v1',
    started_at: new Date().toISOString(), configuration: publicConfiguration(config),
    mock: { delta_count: 256, delta_gap_ms: 10 },
    provenance: { api_binary_sha256: sha256File(config.apiServerBin), packages_sha256: {
      openai: sha256File(config.openaiPackage), anthropic: sha256File(config.anthropicPackage), openai_compatible: sha256File(config.openaiCompatiblePackage),
    } },
    clock_ticks_per_second: clockTicks(), warmup: [], rounds: [], outcome: 'failed',
    limitations: [
      'CPU is observed process CPU seconds, not sampled utilization. Unknown process lifetimes and incomplete enumeration yield null totals.',
      'All API descendants are classified as plugin; short-lived unsampled descendants are unobservable.',
      'RSS/PSS peaks each sum processes within one sequential sample, never sums of individual peaks. Sampling can miss brief peaks.',
      'Harness CPU includes mock, request parsing and sampler; sampler CPU cost is separately bracketed but concurrent callbacks can contribute.',
      'Drain is a bounded observation window, not proof that all background tasks completed.',
      'Database lifecycle belongs to caller; WAL is cluster-wide and enabled only by dedicated-cluster acknowledgment.',
      'Database observations happen outside measured windows; null is missing evidence, never zero.',
    ],
  };
  let fixture, mock;
  let interrupted = false;
  const interrupt = () => { interrupted = true; };
  const check = () => { if (interrupted) throw Error('benchmark interrupted'); };
  fs.mkdirSync(config.artifactRoot, { recursive: true });
  if (fs.existsSync(path.join(config.artifactRoot, 'report.json'))) throw Error('artifact report already exists; choose fresh artifact root');
  process.on('SIGINT', interrupt);
  process.on('SIGTERM', interrupt);
  let phase = 'mock_start';
  try {
    mock = await startMock();
    if ([7600, 7800].includes(Number(new URL(mock.baseUrl).port))) throw Error('protected upstream port');
    let apiPort = config.apiPort;
    if (!apiPort) { do { apiPort = await reserveLoopbackPort(); } while ([7600, 7800].includes(apiPort)); }
    check();
    phase = 'fixture_bootstrap';
    fixture = await createGatewayFixture({ ...config, apiPort, upstreamBaseUrl: mock.baseUrl, artifactRoot: path.join(config.artifactRoot, 'service') }, { OwnerHttpClient: TemporaryOwner });
    const target = fixture.result.targets.openai.gateway;
    const fixtureTarget = fixture.result.targets.openai;
    canaries.push(...Object.values(fixture.result.targets).map(t => t.api_key), ...fixture.result.pools.anthropic.map(t => t.api_key));
    report.api_pid = fixture.gatewayPid;
    fs.writeFileSync(path.join(config.artifactRoot, 'ready.json'), `${JSON.stringify({
      phase: 'ready', gateway_pid: fixture.gatewayPid,
      start_ticks: parseStat(fs.readFileSync(`/proc/${fixture.gatewayPid}/stat`, 'utf8'), fixture.gatewayPid).start_ticks,
      loopback_port: apiPort, api_binary_sha256: report.provenance.api_binary_sha256,
    })}\n`, { mode: 0o600, flag: 'wx' });
    report.database_before = await databaseSnapshot(config);
    const body = requestBody(config.historyBytes, fixtureTarget.model);
    const batch = async () => {
      check();
      const before = mock.snapshot();
      // Wait for every request to settle before cleanup, including rejected requests.
      const results = await Promise.allSettled(Array.from({ length: config.concurrency }, () => request(target, body)));
      const after = mock.snapshot();
      const failure = results.find(r => r.status === 'rejected');
      if (failure) throw failure.reason;
      const requestCount = after.requests - before.requests;
      if (requestCount !== config.concurrency || after.failures !== before.failures) throw Error('upstream request count mismatch');
      return { requests: results.map(r => r.value), upstream: { request_count: requestCount, request_bytes: after.request_bytes - before.request_bytes, response_bytes: after.response_bytes - before.response_bytes, active_at_client_completion: after.active } };
    };
    phase = 'baseline_idle';
    report.idle = await measure(config, fixture.gatewayPid, report.clock_ticks_per_second, () => delay(config.idleMs));
    phase = 'warmup';
    for (let i = 0; i < config.warmups; i++) { check(); report.warmup.push(await batch()); await delay(config.drainMs); }
    for (let i = 0; i < config.rounds; i++) {
      check();
      phase = `round_${i + 1}_workload`;
      const workload = await measure(config, fixture.gatewayPid, report.clock_ticks_per_second, batch);
      phase = `round_${i + 1}_drain`;
      const drain = await measure(config, fixture.gatewayPid, report.clock_ticks_per_second, () => delay(config.drainMs));
      report.rounds.push({ ordinal: i + 1, workload, drain, drain_observation: { window_ms: config.drainMs, upstream: mock.snapshot(), completion_claim: false } });
    }
    report.database_after = await databaseSnapshot(config);
    report.outcome = 'observations_collected';
  } catch (error) {
    report.outcome = interrupted ? 'interrupted' : 'failed';
    report.failed_phase = phase;
    report.error = safeError(error, canaries);
  } finally {
    let cleanupFailed = false;
    report.cleanup_errors = [];
    for (const session of sessions) { try { await session.dispose(); } catch (error) { cleanupFailed = true; report.cleanup_errors.push(safeError(error, canaries)); } }
    try { await fixture?.close(); } catch (error) { cleanupFailed = true; report.cleanup_errors.push(safeError(error, canaries)); }
    try { await mock?.close(); } catch (error) { cleanupFailed = true; report.cleanup_errors.push(safeError(error, canaries)); }
    process.removeListener('SIGINT', interrupt);
    process.removeListener('SIGTERM', interrupt);
    report.cleanup = cleanupFailed ? 'failed' : 'complete';
    if (cleanupFailed) report.outcome = 'failed';
    report.finished_at = new Date().toISOString();
    const encoded = `${JSON.stringify(report, null, 2)}\n`;
    if (canaries.filter(Boolean).some(secret => encoded.includes(secret))) throw Error('report secret exclusion failed');
    const output = path.join(config.artifactRoot, 'report.json');
    fs.writeFileSync(output, encoded, { mode: 0o600, flag: 'wx' });
    assertNoArtifactSecrets([config.artifactRoot], canaries);
  }
  return report.outcome === 'observations_collected';
}
if (require.main === module) {
  Promise.resolve().then(() => run(configuration())).then(ok => {
    console.log(JSON.stringify({ outcome: ok ? 'observations_collected' : 'failed', report: 'report.json' }));
    if (!ok) process.exitCode = 1;
  }).catch(() => { console.error('resource benchmark failed; check required environment and artifact directory'); process.exitCode = 1; });
}
module.exports = { run };
