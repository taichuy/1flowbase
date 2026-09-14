const fs = require('node:fs');
const path = require('node:path');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { buildServiceEnv, resolveCommandPath } = require('./env.js');
const { runPhase } = require('./phases.js');
const { log } = require('./cli.js');

async function buildBackend(service, { runPhaseImpl = runPhase, logImpl = log } = {}) {
  const env = buildServiceEnv(service);
  const config = loadVerifyRuntimeConfig({ repoRoot: service.repoRoot, env }).backend;
  const requestedJobs = Number(env.CARGO_BUILD_JOBS);
  env.CARGO_BUILD_JOBS = String(Number.isInteger(requestedJobs) && requestedJobs > 0 ? Math.min(config.cargoJobs, requestedJobs) : config.cargoJobs);
  if (env.CARGO_INCREMENTAL === undefined && config.incremental !== undefined) env.CARGO_INCREMENTAL = config.incremental ? '1' : '0';
  const cargo = resolveCommandPath('cargo') || 'cargo';
  let executable;
  const args = ['build', '-p', 'api-server', '--bin', 'api-server', '--message-format=json-render-diagnostics'];
  logImpl(`Rust build: cargo=${cargo}; jobs<=${env.CARGO_BUILD_JOBS}; incremental=${env.CARGO_INCREMENTAL ?? 'Cargo default'}`);
  await runPhaseImpl(cargo, args, {
    cwd: service.cwd, env, signal: service.signal,
    logFile: path.join(path.dirname(service.logFile), 'api-server-build.log'),
    label: 'api-server build', timeoutMs: null, logImpl,
    onLine(line) {
      let message;
      try { message = JSON.parse(line); } catch { return; }
      if (message.reason === 'compiler-artifact' && message.target?.name === 'api-server' && message.target.kind?.includes('bin') && message.executable) executable = message.executable;
    },
  });
  if (!executable || !fs.existsSync(executable)) throw new Error('Cargo succeeded without an api-server executable; see api-server-build.log');
  return { command: executable, args: [] };
}

module.exports = { buildBackend };
