'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn } = require('node:child_process');

const CLIENTS = Object.freeze({
  claude: Object.freeze({ binary: 'claude', config: (env, home) => env.CLAUDE_CONFIG_DIR || path.join(home, '.claude') }),
  codex: Object.freeze({ binary: 'codex', config: (env, home) => env.CODEX_HOME || path.join(home, '.codex') }),
  opencode: Object.freeze({
    binary: 'opencode',
    config: (env, home) => env.OPENCODE_CONFIG_DIR
      || path.join(env.XDG_CONFIG_HOME || path.join(home, '.config'), 'opencode'),
  }),
});

function isExecutable(filePath, fsImpl = fs) {
  try {
    return fsImpl.statSync(filePath).isFile()
      && (fsImpl.accessSync(filePath, fs.constants.X_OK), true);
  } catch {
    return false;
  }
}

function findExecutable(name, envPath, fsImpl = fs) {
  if (path.isAbsolute(name)) return isExecutable(name, fsImpl) ? fsImpl.realpathSync(name) : null;
  for (const directory of String(envPath || '').split(path.delimiter).filter(Boolean)) {
    const candidate = path.join(directory, name);
    if (isExecutable(candidate, fsImpl)) return fsImpl.realpathSync(candidate);
  }
  return null;
}

function discoverClients(options = {}) {
  const env = options.env || process.env;
  const home = options.home || env.HOME || os.homedir();
  const fsImpl = options.fsImpl || fs;
  return Object.fromEntries(Object.entries(CLIENTS).map(([client, spec]) => {
    const requestedBinary = options.binaries?.[client] || spec.binary;
    const binary = findExecutable(requestedBinary, env.PATH, fsImpl);
    const configPath = path.resolve(options.configs?.[client] || spec.config(env, home));
    const configExists = fsImpl.existsSync(configPath);
    const status = !binary ? 'skipped' : !configExists ? 'skipped' : 'ready';
    const reason = !binary ? 'binary_not_found' : !configExists ? 'config_not_found' : null;
    return [client, { client, status, reason, binary, config_path: configPath }];
  }));
}

function probeVersion(binary, options = {}) {
  const spawnImpl = options.spawnImpl || spawn;
  const timeoutMs = options.timeoutMs || 5000;
  return new Promise((resolve) => {
    const child = spawnImpl(binary, ['--version'], {
      env: options.env || process.env,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let output = '';
    const append = (chunk) => { output = (output + chunk.toString('utf8')).slice(0, 4096); };
    child.stdout?.on('data', append);
    child.stderr?.on('data', append);
    let settled = false;
    const finish = (result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve(result);
    };
    const timer = setTimeout(() => child.kill('SIGKILL'), timeoutMs);
    child.once('error', (error) => {
      finish({ status: 'failed', version: null, reason: error.message });
    });
    child.once('exit', (code, signal) => {
      const version = output.trim().split(/\r?\n/u)[0] || null;
      finish(code === 0 && version
        ? { status: 'ready', version, reason: null }
        : { status: 'failed', version: null, reason: `version_probe_exit_${code ?? signal}` });
    });
  });
}

module.exports = { CLIENTS, discoverClients, findExecutable, isExecutable, probeVersion };
