'use strict';
// Independent CI only: no local credentials, application history or shared services.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { spawn, execFileSync } = require('node:child_process');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime');
const { parseCargoTestCounts, requireExecutedCargoTests } = require('../verify/cargo-test-results');

const root = path.resolve(__dirname, '../../..');
const out = path.join(root, 'tmp/test-governance/agent-logs-otlp/ci-api');
const receipt = { source_sha: null, result: 'incomplete', commands: [], tests: 0, binary: null };
const profile = ['--config', 'profile.dev.package.api-server.debug=0', '--config', 'profile.test.package.api-server.debug=0'];
const checkpoint = () => fs.writeFileSync(path.join(out, 'receipt.json'), JSON.stringify(receipt, null, 2));

function cpu() {
  return fs.readFileSync('/proc/stat', 'utf8').split('\n')[0].trim().split(/\s+/).slice(1).map(Number);
}
function resources(previous) {
  const current = cpu();
  const delta = current.reduce((n, value, i) => n + value - previous[i], 0);
  const idle = (current[3] - previous[3]) + (current[4] - previous[4]);
  const memory = Object.fromEntries([...fs.readFileSync('/proc/meminfo', 'utf8').matchAll(/^(\w+):\s+(\d+)/gm)].map(m => [m[1], Number(m[2])]));
  const disk = fs.statfsSync(root);
  return { current, sample: {
    cpu_fraction: delta > 0 ? 1 - idle / delta : 0,
    memory_fraction: 1 - memory.MemAvailable / memory.MemTotal,
    disk_fraction: 1 - disk.bavail / disk.blocks
  }};
}

async function command(label, args, env) {
  const log = path.join(out, `${label}.log`);
  const fd = fs.openSync(log, 'w');
  const child = spawn('cargo', args, { cwd: root, env, detached: true, stdio: ['ignore', fd, fd] });
  let previous = cpu(), stopped = false, killTimer;
  const item = { label, command: ['cargo', ...args], exit_code: null, signal: null, log: path.basename(log) };
  receipt.commands.push(item);
  checkpoint();
  const monitor = setInterval(() => {
    const observed = resources(previous);
    previous = observed.current;
    fs.appendFileSync(path.join(out, 'resources.jsonl'), JSON.stringify({ label, ...observed.sample }) + '\n');
    if (!stopped && Object.values(observed.sample).some(value => value >= 0.9)) {
      stopped = true;
      item.resource_stop = observed.sample;
      try { process.kill(-child.pid, 'SIGTERM'); } catch { /* child may already have finished */ }
      killTimer = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch {} }, 10000);
    }
  }, 2000);
  try {
    await new Promise((resolve, reject) => {
      child.once('error', reject);
      child.once('exit', (code, signal) => {
        item.exit_code = code;
        item.signal = signal;
        if (code === 0 && !stopped) resolve();
        else reject(new Error(`${label} failed; see ${item.log}`));
      });
    });
  } finally {
    clearInterval(monitor);
    if (stopped) { try { process.kill(-child.pid, 'SIGKILL'); } catch {} }
    clearTimeout(killTimer);
    fs.closeSync(fd);
    checkpoint();
  }
  return log;
}

function buildApiTestCommands(cargoTestThreads) {
  const api = ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'api-server', '--features', 'tikv-jemallocator/stats', ...profile];
  // Entire named modules, plus directly affected native ingress/archive/registration routes.
  const selectors = [
    ['codec', 'agent_logs_protocol::'],
    ['public-contract', 'routes::application_public_api::agent_logs::otlp::'],
    ['public-route', 'application_api_routes::otlp_logs::', 'api_application'],
    ['console-contract', 'application_runtime::log_exchange::'],
    ['unified-exchange', 'application_log_exchange::'],
    ['exchange-host-api', 'exchange_host_services'],
    ['exchange-host-graph', 'exchange_host_services', 'extension_graph_contract'],
    ['exchange-session', 'read_and_import_session_currentness'],
    ['console-route', 'logs_routes::exchange::', 'api_application'],
    ['openapi', 'canonical_otlp_trace_ingest_advertises_application_bearer_and_standard_wire_formats', 'api_documentation'],
    ['native-reception', 'application_api_routes::log_reception::', 'api_application'],
    ['native-export', 'application_runtime_routes_logs_export_json_trace_dump_preserves_detail_and_error_payload', 'api_application'],
    ['native-archive', 'application_runtime_routes_logs_archive_returns_v1_manifest_and_restore_facts', 'api_application'],
    ['operation-registry', 'console_route_assembly'],
    ['operation-migration', 'console_policy_migration']
  ];
  return selectors.map(([label, selector, target]) => ({
    label,
    args: [...api, ...(target ? ['--test', target] : ['--lib']), selector, '--', '--test-threads', String(cargoTestThreads)],
  }));
}

async function main() {
  assert.equal(process.env.GITHUB_ACTIONS, 'true', 'CI verifier must run on its dedicated GitHub runner');
  fs.mkdirSync(out, { recursive: true });
  const sha = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
  assert.match(process.env.EXPECTED_SOURCE_SHA || '', /^[a-f0-9]{40}$/);
  assert.equal(sha, process.env.EXPECTED_SOURCE_SHA);
  receipt.source_sha = sha;
  receipt.rustc = execFileSync('rustc', ['--version'], { encoding: 'utf8' }).trim();
  const actualParallelism = os.availableParallelism();
  const allowed = fs.readFileSync('/proc/self/status', 'utf8').match(/^Cpus_allowed_list:\s*(.+)$/m);
  assert.ok(allowed, 'Linux CPU affinity must be available');
  const cpuIds = allowed[1].trim().split(',').flatMap(part => {
    const [first, last = first] = part.split('-').map(Number);
    return Array.from({ length: last - first + 1 }, (_, i) => first + i);
  });
  // Route fixtures also start Tokio/background/database work. Reserve CPU for
  // those owners using Linux affinity, inherited by our Cargo/test children.
  const verifyCapacity = Math.max(1, Math.floor(Math.min(actualParallelism, cpuIds.length) / 2));
  const affinity = cpuIds.slice(0, verifyCapacity).join(',');
  execFileSync('taskset', ['-pc', affinity, String(process.pid)], { stdio: 'pipe' });
  const budget = loadVerifyRuntimeConfig({ repoRoot: root, availableParallelism: verifyCapacity }).backend;
  receipt.resource_budget = { actual_parallelism: actualParallelism, cpu_affinity: affinity, ...budget };
  const env = { ...process.env, CARGO_BUILD_JOBS: String(budget.cargoJobs), CARGO_INCREMENTAL: '0' };
  // Keep archive decompression/network downloads separate from compilation.
  await command('fetch', ['fetch', '--locked', '--manifest-path', 'api/Cargo.toml'], env);
  // Match the current protected baseline's API gate fixture setup. Preparing
  // workers once avoids nested Cargo from mounted notification/catalog tests.
  await command('notification-workers', ['build', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'runtime-extension-sdk', '--example', 'notification_worker', '--example', 'notification_authorization_worker', ...profile], env);
  env.NOTIFICATION_WORKER_FIXTURE = path.join(root, 'api/target/debug/examples/notification_worker');
  env.NOTIFICATION_AUTHORIZATION_WORKER_FIXTURE = path.join(root, 'api/target/debug/examples/notification_authorization_worker');
  const testFailures = [];
  async function testCommand(label, args) {
    let log;
    try {
      log = await command(label, args, env);
    } catch (error) {
      const item = receipt.commands.at(-1);
      const output = fs.readFileSync(path.join(out, item.log), 'utf8');
      const counts = parseCargoTestCounts(output);
      // Collect completed behavior failures across the finite batch. Compile,
      // signal and resource failures stop immediately; they cannot be treated
      // as completed test evidence or safely repeated by another selector.
      if (item.resource_stop || item.signal || counts.failedCount === null) throw error;
      item.counts = counts;
      receipt.tests += counts.passedCount;
      testFailures.push(label);
      checkpoint();
      process.stdout.write(JSON.stringify({ label, ...counts }) + '\n');
      return;
    }
    const counts = requireExecutedCargoTests(fs.readFileSync(log, 'utf8'));
    receipt.commands.at(-1).counts = counts;
    receipt.tests += counts.passedCount;
    checkpoint();
    process.stdout.write(JSON.stringify({ label, ...counts }) + '\n');
  }
  for (const { label, args } of buildApiTestCommands(budget.cargoTestThreads)) {
    await testCommand(label, args);
  }
  for (const [label, cargoArgs] of [
    ['exchange-contracts', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'extension-contracts', '--lib', 'exchange_host_services']],
    ['exchange-sdk', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'runtime-extension-sdk', '--lib', 'exchange_host_services']],
    ['exchange-host', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'runtime-extension-host', '--lib', 'exchange_host_services']],
    ['exchange-authority', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'control-plane', '--lib', 'exchange_host_services']],
    ['exchange-egress-domain', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'domain', '--lib', 'managed_plugin_selector']],
    ['source-contracts', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'control-plane-contracts', '--lib', 'agent_logs_contract_tests']],
    ['native-source-units', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'storage-durable-postgres', '--lib', 'native_log_exchange']],
    ['selected-source-snapshot', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'storage-durable-postgres', '--lib', 'agent_logs::reader::selected_facts']],
    ['independent-grants', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'control-plane', '--lib', 'agent_log_exchange_operations_are_independent_and_do_not_inherit_archive_grants']],
    ['pricing', ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'control-plane-postgres-tests', '--test', 'orchestration_runtime_integration', 'application_logs::agent_logs::pricing']]
  ]) {
    await testCommand(label, [...cargoArgs, ...profile, '--', '--test-threads', String(budget.cargoTestThreads)]);
  }
  receipt.failed_selectors = testFailures;
  checkpoint();
  assert.equal(testFailures.length, 0, `completed test failures: ${testFailures.join(', ')}`);
  await command('runtime-build', ['build', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'api-server', '--bin', 'api-server', '--features', 'tikv-jemallocator/stats', ...profile], env);
  const binary = path.join(out, 'api-server');
  fs.copyFileSync(path.join(root, 'api/target/debug/api-server'), binary);
  fs.chmodSync(binary, 0o755);
  receipt.binary = { filename: 'api-server', sha256: crypto.createHash('sha256').update(fs.readFileSync(binary)).digest('hex'), source_sha: sha, platform: process.platform, arch: process.arch, features: ['tikv-jemallocator/stats'] };
  receipt.result = 'pass';
  checkpoint();
  process.stdout.write(JSON.stringify({ result: receipt.result, source_sha: sha, tests: receipt.tests, binary: receipt.binary }) + '\n');
}

if (require.main === module) main().catch(error => {
  receipt.result = 'fail';
  if (fs.existsSync(out)) checkpoint();
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
});

module.exports = { buildApiTestCommands };
