const fs = require('node:fs');
const path = require('node:path');

const { parseEnvFile } = require('./env.js');

const DEFAULT_STARTUP_TIMEOUT_MS = 15_000;
const FRONTEND_COLD_STARTUP_TIMEOUT_MS = 60_000;
const CARGO_COLD_STARTUP_TIMEOUT_MS = 60_000;
const DEFAULT_WEB_PORT = 3100;
const DEFAULT_API_SERVER_PORT = 7800;
const DEFAULT_PLUGIN_RUNNER_PORT = 7801;
const DEFAULT_API_SERVER_ADDR = '0.0.0.0:7800';
const DEFAULT_PLUGIN_RUNNER_ADDR = '0.0.0.0:7801';

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function getRuntimePaths(repoRoot) {
  const tmpDir = path.join(repoRoot, 'tmp', 'dev-up');
  const pidDir = path.join(tmpDir, 'pids');
  const logDir = path.join(repoRoot, 'tmp', 'logs');

  return {
    tmpDir,
    pidDir,
    logDir,
  };
}

function ensureRuntimeDirs(paths) {
  fs.mkdirSync(paths.tmpDir, { recursive: true });
  fs.mkdirSync(paths.pidDir, { recursive: true });
  fs.mkdirSync(paths.logDir, { recursive: true });
}

function parsePositivePort(value, fallback) {
  const parsed = Number.parseInt(String(value || '').trim(), 10);
  return Number.isInteger(parsed) && parsed > 0 && parsed <= 65_535 ? parsed : fallback;
}

function parseDefaultServiceAddress(value, fallbackPort) {
  const [bindHost, port] = String(value).split(':');
  return {
    bindHost: bindHost || '0.0.0.0',
    port: parsePositivePort(port, fallbackPort),
  };
}

function parseServiceAddress(value, fallback, fallbackPort) {
  const candidate = String(value || fallback).trim();
  const fallbackAddress = parseDefaultServiceAddress(fallback, fallbackPort);
  try {
    const parsed = new URL(`http://${candidate}`);
    const port = parsePositivePort(parsed.port, fallbackAddress.port);
    return {
      bindHost: parsed.hostname,
      port,
    };
  } catch (_error) {
    return fallbackAddress;
  }
}

function getServiceDefinitions(repoRoot) {
  const paths = getRuntimePaths(repoRoot);
  const apiServerEnvDir = path.join(repoRoot, 'api', 'apps', 'api-server');
  const apiServerEnvFile = path.join(apiServerEnvDir, '.env');
  const apiServerEnvExampleFile = path.join(apiServerEnvDir, '.env.example');
  const localEnv = parseEnvFile(apiServerEnvFile);
  const apiServerAddress = parseServiceAddress(
    localEnv.API_SERVER_ADDR,
    DEFAULT_API_SERVER_ADDR,
    DEFAULT_API_SERVER_PORT
  );
  const pluginRunnerAddress = parseServiceAddress(
    localEnv.PLUGIN_RUNNER_ADDR,
    DEFAULT_PLUGIN_RUNNER_ADDR,
    DEFAULT_PLUGIN_RUNNER_PORT
  );
  const webPort = parsePositivePort(localEnv.VITE_DEV_SERVER_PORT, DEFAULT_WEB_PORT);

  return {
    web: {
      key: 'web',
      label: 'frontend',
      repoRoot,
      cwd: path.join(repoRoot, 'web'),
      command: 'pnpm',
      args: ['--filter', '@1flowbase/web', 'dev'],
      bindHost: '0.0.0.0',
      probeHost: '127.0.0.1',
      port: webPort,
      startupTimeoutMs: FRONTEND_COLD_STARTUP_TIMEOUT_MS,
      envFile: apiServerEnvFile,
      logFile: path.join(paths.logDir, 'web.log'),
      pidFile: path.join(paths.pidDir, 'web.json'),
    },
    'api-server': {
      key: 'api-server',
      label: 'api-server',
      repoRoot,
      cwd: path.join(repoRoot, 'api'),
      command: 'cargo',
      args: ['run', '-p', 'api-server', '--bin', 'api-server'],
      bindHost: apiServerAddress.bindHost,
      probeHost: '127.0.0.1',
      port: apiServerAddress.port,
      startupTimeoutMs: CARGO_COLD_STARTUP_TIMEOUT_MS,
      envFile: apiServerEnvFile,
      envExampleFile: apiServerEnvExampleFile,
      logFile: path.join(paths.logDir, 'api-server.log'),
      pidFile: path.join(paths.pidDir, 'api-server.json'),
    },
    'plugin-runner': {
      key: 'plugin-runner',
      label: 'plugin-runner',
      repoRoot,
      cwd: path.join(repoRoot, 'api'),
      command: 'cargo',
      args: ['run', '-p', 'plugin-runner', '--bin', 'plugin-runner'],
      bindHost: pluginRunnerAddress.bindHost,
      probeHost: '127.0.0.1',
      port: pluginRunnerAddress.port,
      startupTimeoutMs: CARGO_COLD_STARTUP_TIMEOUT_MS,
      envFile: apiServerEnvFile,
      logFile: path.join(paths.logDir, 'plugin-runner.log'),
      pidFile: path.join(paths.pidDir, 'plugin-runner.json'),
    },
  };
}

module.exports = {
  CARGO_COLD_STARTUP_TIMEOUT_MS,
  DEFAULT_STARTUP_TIMEOUT_MS,
  FRONTEND_COLD_STARTUP_TIMEOUT_MS,
  ensureRuntimeDirs,
  getRepoRoot,
  getRuntimePaths,
  getServiceDefinitions,
};
