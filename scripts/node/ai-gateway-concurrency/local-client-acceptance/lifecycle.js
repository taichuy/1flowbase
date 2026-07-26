'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');

const ENVIRONMENT_ALLOWLIST = Object.freeze([
  'PATH', 'LANG', 'LC_ALL', 'TZ', 'SSL_CERT_FILE', 'SSL_CERT_DIR', 'NODE_EXTRA_CA_CERTS',
]);

class OwnedResources {
  constructor(dependencies = {}) {
    this.children = new Set();
    this.tmuxSockets = new Set();
    this.tempRoots = new Set();
    this.spawnSync = dependencies.spawnSync || spawnSync;
    this.rmSync = dependencies.rmSync || fs.rmSync;
  }

  addChild(child) { this.children.add(child); return child; }
  releaseChild(child) { this.children.delete(child); }
  addTmuxSocket(socket) { this.tmuxSockets.add(socket); return socket; }
  releaseTmuxSocket(socket) { this.tmuxSockets.delete(socket); }
  addTempRoot(root) { this.tempRoots.add(root); return root; }

  async close() {
    const errors = [];
    for (const child of this.children) {
      try {
        if (child.exitCode === null && child.signalCode === null) child.kill('SIGTERM');
        await Promise.race([
          new Promise((resolve) => child.once?.('exit', resolve)),
          new Promise((resolve) => setTimeout(resolve, 500)),
        ]);
        if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
      } catch (error) { errors.push({ owner: 'child', message: error.message }); }
    }
    this.children.clear();
    for (const socket of this.tmuxSockets) {
      try {
        const result = this.spawnSync('tmux', ['-L', socket, 'kill-server'], { stdio: 'ignore' });
        if (result?.error) throw result.error;
      } catch (error) { errors.push({ owner: `tmux:${socket}`, message: error.message }); }
    }
    this.tmuxSockets.clear();
    for (const root of this.tempRoots) {
      try { this.rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 20 }); }
      catch (error) { errors.push({ owner: `temp:${root}`, message: error.message }); }
    }
    this.tempRoots.clear();
    return errors;
  }
}

function shellQuote(value) {
  return `'${String(value).replaceAll("'", `'\\''`)}'`;
}

function waitForFile(filePath, timeoutMs) {
  if (fs.existsSync(filePath)) return Promise.resolve();
  return new Promise((resolve, reject) => {
    let watcher;
    let settled = false;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      watcher?.close();
      if (error) reject(error); else resolve();
    };
    const timer = setTimeout(() => finish(new Error('tmux client invocation timed out')), timeoutMs);
    watcher = fs.watch(path.dirname(filePath), () => {
      if (fs.existsSync(filePath)) finish();
    });
    if (fs.existsSync(filePath)) finish();
  });
}

function readBoundedFile(filePath, maximumBytes = 1024 * 1024) {
  if (!fs.existsSync(filePath)) return '';
  const descriptor = fs.openSync(filePath, 'r');
  try {
    const size = Math.min(fs.fstatSync(descriptor).size, maximumBytes);
    const buffer = Buffer.alloc(size);
    fs.readSync(descriptor, buffer, 0, size, 0);
    return buffer.toString('utf8');
  } finally {
    fs.closeSync(descriptor);
  }
}

function executionEnvironment(plan, parentEnv = process.env) {
  const environment = {};
  for (const name of ENVIRONMENT_ALLOWLIST) {
    if (typeof parentEnv[name] === 'string' && parentEnv[name] !== '') environment[name] = parentEnv[name];
  }
  const isolatedHome = path.dirname(plan.invocation.cwd);
  return {
    ...environment,
    HOME: isolatedHome,
    USERPROFILE: isolatedHome,
    TMPDIR: isolatedHome,
    NO_PROXY: '127.0.0.1,localhost,::1',
    no_proxy: '127.0.0.1,localhost,::1',
    ...plan.environment,
  };
}

function executeChild(plan, options = {}) {
  const registry = options.registry;
  const spawnImpl = options.spawnImpl || spawn;
  const timeoutMs = options.timeoutMs || 180000;
  return new Promise((resolve) => {
    const child = registry.addChild(spawnImpl(plan.invocation.executable, plan.invocation.args, {
      cwd: plan.invocation.cwd,
      env: executionEnvironment(plan, options.parentEnv),
      stdio: ['ignore', 'pipe', 'pipe'],
    }));
    let stdout = '';
    let stderr = '';
    const append = (target, chunk) => `${target}${chunk.toString('utf8')}`.slice(0, 1024 * 1024);
    child.stdout?.on('data', (chunk) => { stdout = append(stdout, chunk); });
    child.stderr?.on('data', (chunk) => { stderr = append(stderr, chunk); });
    let timedOut = false;
    const timer = setTimeout(() => { timedOut = true; child.kill('SIGKILL'); }, timeoutMs);
    child.once('error', (error) => {
      clearTimeout(timer);
      registry.releaseChild(child);
      resolve({ exit_code: null, signal: null, timed_out: false, stdout, stderr: error.message });
    });
    child.once('exit', (code, signal) => {
      clearTimeout(timer);
      registry.releaseChild(child);
      resolve({ exit_code: code, signal, timed_out: timedOut, stdout, stderr });
    });
  });
}

async function executeTmux(plan, options = {}) {
  const registry = options.registry;
  const spawnImpl = options.spawnImpl || spawn;
  const timeoutMs = options.timeoutMs || 180000;
  const tmux = options.tmuxExecutable || 'tmux';
  const root = registry.addTempRoot(fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-client-tmux-')));
  const stdoutPath = path.join(root, 'stdout.log');
  const stderrPath = path.join(root, 'stderr.log');
  const statusPath = path.join(root, 'status');
  const socket = registry.addTmuxSocket(`1flowbase-local-client-${process.pid}-${path.basename(root)}`);
  const command = [plan.invocation.executable, ...plan.invocation.args].map(shellQuote).join(' ');
  const shellCommand = [
    `${command} >${shellQuote(stdoutPath)} 2>${shellQuote(stderrPath)}`,
    'status=$?',
    `printf '%s\\n' "$status" >${shellQuote(statusPath)}`,
  ].join('; ');
  let outputWatcher = null;
  let barrierRelease = null;
  let markerObserved = false;
  const launched = await new Promise((resolve) => {
    const child = registry.addChild(spawnImpl(tmux, [
      '-L', socket, 'new-session', '-d', '-s', 'local-client', '-c', plan.invocation.cwd, shellCommand,
    ], {
      env: executionEnvironment(plan, options.parentEnv),
      stdio: ['ignore', 'pipe', 'pipe'],
    }));
    let stderr = '';
    child.stderr?.on('data', (chunk) => { stderr = `${stderr}${chunk.toString('utf8')}`.slice(0, 65536); });
    child.once('error', (error) => {
      registry.releaseChild(child);
      resolve({ ok: false, error: error.message });
    });
    child.once('exit', (code) => {
      registry.releaseChild(child);
      resolve(code === 0 ? { ok: true } : { ok: false, error: stderr || `tmux exited with ${code}` });
    });
  });
  if (!launched.ok) {
    return { exit_code: null, signal: null, timed_out: false, stdout: '', stderr: launched.error };
  }
  if (typeof options.onFirstMarker === 'function') {
    outputWatcher = fs.watch(root, () => {
      if (markerObserved || !fs.existsSync(stdoutPath)) return;
      let output;
      try { output = readBoundedFile(stdoutPath); } catch { return; }
      if (!output.includes('marker-1')) return;
      markerObserved = true;
      barrierRelease = Promise.resolve().then(() => options.onFirstMarker());
    });
  }
  let timedOut = false;
  try {
    await waitForFile(statusPath, timeoutMs);
    await barrierRelease;
  } catch (error) {
    timedOut = true;
    return { exit_code: null, signal: 'SIGKILL', timed_out: true, stdout: '', stderr: error.message };
  } finally {
    outputWatcher?.close();
    const result = registry.spawnSync(tmux, ['-L', socket, 'kill-server'], { stdio: 'ignore' });
    if (!result?.error) registry.releaseTmuxSocket(socket);
  }
  return {
    exit_code: Number.parseInt(fs.readFileSync(statusPath, 'utf8').trim(), 10),
    signal: null,
    timed_out: timedOut,
    stdout: readBoundedFile(stdoutPath),
    stderr: readBoundedFile(stderrPath),
  };
}

module.exports = {
  ENVIRONMENT_ALLOWLIST,
  OwnedResources,
  executeChild,
  executeTmux,
  executionEnvironment,
  readBoundedFile,
  shellQuote,
  waitForFile,
};
