const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const { performance } = require('node:perf_hooks');
const { spawn, execFileSync } = require('node:child_process');
const { isCiEnvironment } = require('../testing/verify-runtime.js');

const MARKER = 'ONEFLOWBASE_RESET_SCOPE';
const CGROOT = '/sys/fs/cgroup';
const UNIT = /^rust-cache-reset-\d+-[a-f0-9]{24}\.scope$/;
const defaults = {
  read: (file) => fs.readFileSync(file, 'utf8').trim(),
  systemctl: (args) => execFileSync('/usr/bin/systemctl', ['--user', ...args], {
    encoding: 'utf8', timeout: 10000, maxBuffer: 16384, stdio: ['ignore', 'pipe', 'pipe'],
  }).trim(),
  spawn, process, os, setInterval, clearInterval, setTimeout, clearTimeout,
  nonce: () => crypto.randomBytes(12).toString('hex'),
  now: () => performance.now(),
};

function cgroupPath(relative) {
  if (!relative.startsWith('/') || relative.split('/').includes('..')) throw new Error('Invalid cgroup path');
  return path.join(CGROOT, relative);
}

function hierarchy(relative, read) {
  const result = [];
  for (let current = cgroupPath(relative); current !== CGROOT; current = path.dirname(current)) {
    const cpu = read(path.join(current, 'cpu.max')).split(/\s+/);
    const memory = read(path.join(current, 'memory.max'));
    const swap = read(path.join(current, 'memory.swap.max'));
    const cpuMax = cpu[0] === 'max' ? Infinity : Number(cpu[0]) / Number(cpu[1]);
    const memoryMax = memory === 'max' ? Infinity : Number(memory);
    const swapMax = swap === 'max' ? Infinity : Number(swap);
    if (!(cpuMax > 0) || !(memoryMax > 0) || !(swapMax >= 0)) {
      throw new Error('Missing or invalid cgroup CPU/memory/swap controller support');
    }
    result.push({ cpuMax, memoryMax, swapMax });
  }
  if (!result.length) throw new Error('Missing user cgroup hierarchy');
  return {
    cpu: Math.min(...result.map((v) => v.cpuMax)),
    memory: Math.min(...result.map((v) => v.memoryMax)),
    swap: Math.min(...result.map((v) => v.swapMax)),
    leaf: result[0],
  };
}

function currentCgroup(read) {
  const entry = read('/proc/self/cgroup').split('\n').find((line) => line.startsWith('0::'));
  if (!entry) throw new Error('Linux cgroup v2 is required before cache cleanup');
  return entry.slice(3);
}

function budgets({ env, runtime, inherited = { cpu: Infinity, memory: Infinity } }) {
  const { os: host } = runtime;
  const explicit = env.ONEFLOWBASE_CARGO_JOB_LIMIT;
  if (explicit !== undefined && explicit !== '' && explicit !== 'auto' && !/^[1-9][0-9]*$/.test(explicit)) {
    throw new Error('ONEFLOWBASE_CARGO_JOB_LIMIT must be a positive integer or auto');
  }
  const cpu = Math.min(host.cpus().length / 2,
    host.availableParallelism?.() ?? host.cpus().length,
    explicit && explicit !== 'auto' ? Number(explicit) : Infinity, inherited.cpu);
  const memory = Math.floor(Math.min(host.totalmem() / 2, inherited.memory));
  if (!(cpu > 0) || !Number.isSafeInteger(memory) || memory < 1048576) throw new Error('Invalid half-host resource budget');
  return { cpu, memory };
}

function verifyScope(marker, env, runtime) {
  let owner;
  try { owner = JSON.parse(marker); } catch { throw new Error('Invalid reset resource scope marker'); }
  if (!owner || !UNIT.test(owner.unit) || typeof owner.parent !== 'string' ||
      path.posix.basename(owner.parent) !== 'dev-rust.slice') throw new Error('Invalid owned reset resource scope');
  const expected = owner.parent + '/' + owner.unit;
  if (currentCgroup(runtime.read) !== expected) throw new Error('Reset scope marker does not match actual cgroup');
  const actual = hierarchy(expected, runtime.read);
  const cap = budgets({ env, runtime });
  if (!(owner.cpu > 0 && owner.cpu <= cap.cpu) || !(owner.memory > 0 && owner.memory <= cap.memory) ||
      actual.leaf.cpuMax > owner.cpu + 0.000001 || actual.leaf.memoryMax > owner.memory ||
      actual.leaf.swapMax !== 0 || actual.cpu > cap.cpu || actual.memory > cap.memory) {
    throw new Error('Reset scope CPU/memory/swap limits failed verification');
  }
  return { ...owner, cpu: actual.cpu, memory: actual.memory };
}

function pressureMonitor(read, scope, { now = () => performance.now(), warn = () => {} } = {}) {
  let low = false;
  let lastTime;
  let avgHits = 0;
  const sources = {};
  const full = (file) => {
    const lines = read(file).split('\n').filter((line) => /^full(?:\s|$)/.test(line));
    if (lines.length !== 1) throw new Error('Cannot monitor memory PSI full data');
    const fields = lines[0].trim().split(/\s+/).slice(1);
    const number = (key, pattern) => {
      const values = fields.filter((field) => field.startsWith(key + '='));
      const value = values[0]?.slice(key.length + 1);
      if (values.length !== 1 || !pattern.test(value)) throw new Error('Cannot monitor memory PSI ' + key);
      return Number(value);
    };
    const avg10 = number('avg10', /^\d+(?:\.\d+)?$/);
    const total = number('total', /^\d+$/);
    if (!Number.isFinite(avg10) || avg10 < 0 || avg10 > 100 || !Number.isSafeInteger(total)) {
      throw new Error('Cannot monitor memory PSI avg10/total values');
    }
    return { avg10, total };
  };
  const observe = (name, value, time) => {
    if (!sources[name]) {
      sources[name] = { lastObservedTotal: value.total, anchorTime: time, anchorTotal: value.total, windowHits: 0 };
      return;
    }
    const source = sources[name];
    if (value.total < source.lastObservedTotal) throw new Error('Memory PSI ' + name + ' total regressed');
    source.lastObservedTotal = value.total;
    const dt = time - source.anchorTime;
    // Keep both the anchor and count through short observations, including timer jitter.
    if (dt >= 1000) {
      const percent = (value.total - source.anchorTotal) / (dt * 1000) * 100;
      if (!Number.isFinite(percent)) throw new Error('Cannot monitor memory PSI window');
      source.windowHits = percent >= 20 ? source.windowHits + 1 : 0;
      source.anchorTime = time;
      source.anchorTotal = value.total;
    }
  };
  return () => {
    const time = now();
    if (!Number.isFinite(time) || time < 0 || (lastTime !== undefined && time <= lastTime)) {
      throw new Error('Invalid or non-increasing resource monitor clock');
    }
    const memory = read('/proc/meminfo').split('\n');
    const amount = (key) => {
      const lines = memory.filter((line) => line.startsWith(key + ':'));
      const match = lines.length === 1 && lines[0].match(new RegExp('^' + key + ':\\s*(\\d+)[ \\t]+kB[ \\t]*$'));
      const value = match && Number(match[1]);
      if (!match || !Number.isSafeInteger(value)) throw new Error('Cannot monitor host memory availability');
      return value;
    };
    const total = amount('MemTotal');
    const available = amount('MemAvailable');
    if (total <= 0) throw new Error('Cannot monitor host memory availability');
    const hostFull = full('/proc/pressure/memory');
    let scopeFull;
    if (scope) {
      try { scopeFull = full(path.join(cgroupPath(scope), 'memory.pressure')); }
      catch (error) { if (error.code !== 'ENOENT' && error.code !== 'ENODEV') throw error; }
    }
    observe('host', hostFull, time);
    if (scopeFull) observe('scope', scopeFull, time);
    avgHits = Math.max(hostFull.avg10, scopeFull?.avg10 ?? 0) >= 20 ? avgHits + 1 : 0;
    const nextLow = available < total * 0.1;
    if (nextLow !== low) warn(nextLow
      ? 'host MemAvailable below 10%; continuing with memory PSI monitoring'
      : 'host MemAvailable recovered to at least 10%');
    low = nextLow;
    lastTime = time;
    if (avgHits >= 4) return 'memory full PSI avg10 >=20% for 4 consecutive samples';
    if (Math.max(sources.host.windowHits, sources.scope?.windowHits ?? 0) >= 4) {
      return 'memory full PSI total >=20% for 4 consecutive complete windows';
    }
    return null;
  };
}

function drainScope(unit, relative, runtime) {
  if (!UNIT.test(unit) || path.posix.basename(relative) !== unit) throw new Error('Refusing to drain unowned scope');
  // A unique owned scope is the only signal target, on success as well as failure.
  for (const args of [
    ['kill', '--kill-whom=all', '--signal=TERM', unit],
    ['stop', unit],
    ['kill', '--kill-whom=all', '--signal=KILL', unit],
  ]) {
    try { runtime.systemctl(args); } catch { /* A scope vanishes normally after its last process exits. */ }
  }
  try {
    if (/\bpopulated 1\b/.test(runtime.read(path.join(cgroupPath(relative), 'cgroup.events')))) {
      throw new Error('Owned reset scope still has live processes after drain');
    }
  } catch (error) {
    if (error.code !== 'ENOENT' && error.code !== 'ENODEV') throw error;
  }
}

async function runInResourceScope({
  argv, run, cliPath = path.join(__dirname, 'cli.js'),
  env = process.env, platform = process.platform,
  write = (text) => process.stdout.write(text), runtime: supplied = {},
}) {
  const runtime = { ...defaults, ...supplied };
  // Markers are always verified, including in CI. The public Cargo flag is never proof.
  if (env[MARKER]) {
    const owner = verifyScope(env[MARKER], env, runtime);
    write('[1flowbase-reset-rust-cache] verified ' + owner.unit +
      ' CPU=' + owner.cpu + ' cores MemoryMax=' + owner.memory + ' swap=0; cleanup/warmup starting\n');
    env.CARGO_MEMORY_BUDGET_ACTIVE = '1';
    return run();
  }
  if (platform !== 'linux' || isCiEnvironment(env)) return run();
  const parent = runtime.systemctl(['show', 'dev-rust.slice', '--property=ControlGroup', '--value']);
  if (path.posix.basename(parent) !== 'dev-rust.slice') throw new Error('Existing dev-rust.slice is required before cache cleanup');
  const parentLimits = hierarchy(parent, runtime.read);
  const callerLimits = hierarchy(currentCgroup(runtime.read), runtime.read);
  const cap = budgets({ env, runtime, inherited: {
    cpu: Math.min(parentLimits.cpu, callerLimits.cpu),
    memory: Math.min(parentLimits.memory, callerLimits.memory),
  } });
  const unit = 'rust-cache-reset-' + runtime.process.pid + '-' + runtime.nonce() + '.scope';
  const relative = parent + '/' + unit;
  const owner = { unit, parent, ...cap };
  const monitor = pressureMonitor(runtime.read, relative, {
    now: runtime.now, warn: (message) => write('[1flowbase-reset-rust-cache] warning: ' + message + '\n'),
  });
  // Check monitoring support before starting a scope or touching cache/services.
  monitor();
  write('[1flowbase-reset-rust-cache] creating ' + unit +
    ' beneath dev-rust.slice CPU=' + cap.cpu + ' cores MemoryMax=' + cap.memory + ' swap=0\n');
  const childEnv = { ...env, [MARKER]: JSON.stringify(owner) };
  delete childEnv.CARGO_MEMORY_BUDGET_ACTIVE;
  const args = ['--user', '--scope', '--quiet', '--unit=' + unit, '--slice=dev-rust.slice',
    '--property=CPUQuota=' + (cap.cpu * 100) + '%', '--property=MemoryMax=' + cap.memory,
    '--property=MemoryHigh=' + Math.floor(cap.memory * 0.9), '--property=MemorySwapMax=0',
    '--property=TimeoutStopSec=3s', '--', runtime.process.execPath, cliPath, ...argv];
  let child;
  let interval;
  let reason;
  let forcedStatus;
  let drained = false;
  let launcherDeadline;
  const drain = () => { if (!drained) { drainScope(unit, relative, runtime); drained = true; } };
  const abort = (message, status) => {
    if (reason) return;
    reason = message; forcedStatus = status;
    write('[1flowbase-reset-rust-cache] abort: ' + message + '; draining ' + unit + '\n');
    try { drain(); } catch (error) {
      write('[1flowbase-reset-rust-cache] scope drain failed: ' + error.message + '\n');
    }
    launcherDeadline = runtime.setTimeout(() => child?.kill('SIGKILL'), 5000);
    // This is the launcher we created, never a discovered PID or unrelated process.
    child?.kill('SIGTERM');
  };
  const interrupt = () => abort('SIGINT', 130);
  const terminate = () => abort('SIGTERM', 143);
  try {
    child = runtime.spawn('/usr/bin/systemd-run', args, { env: childEnv, stdio: 'inherit' });
    runtime.process.on('SIGINT', interrupt);
    runtime.process.on('SIGTERM', terminate);
    interval = runtime.setInterval(() => {
      try {
        const message = monitor();
        if (message) abort(message, 1);
      } catch (error) { abort('resource monitor failed: ' + error.message, 1); }
    }, 1000);
    const status = await new Promise((resolve, reject) => {
      child.once('error', reject);
      child.once('close', (code, signal) => resolve(code ?? (signal === 'SIGINT' ? 130 : signal === 'SIGTERM' ? 143 : 1)));
    });
    drain();
    write('[1flowbase-reset-rust-cache] scope finished status=' + (forcedStatus ?? status) + '\n');
    return forcedStatus ?? status;
  } finally {
    if (interval !== undefined) runtime.clearInterval(interval);
    if (launcherDeadline !== undefined) runtime.clearTimeout(launcherDeadline);
    runtime.process.removeListener('SIGINT', interrupt);
    runtime.process.removeListener('SIGTERM', terminate);
    drain();
  }
}

module.exports = { MARKER, budgets, hierarchy, verifyScope, pressureMonitor, drainScope, runInResourceScope };
