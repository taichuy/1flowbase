const fs = require('node:fs');
const path = require('node:path');
const net = require('node:net');

function canonicalPath(value, fsImpl = fs) {
  try {
    return fsImpl.realpathSync(value);
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    const parent = path.dirname(value);
    if (parent === value) throw error;
    return path.join(canonicalPath(parent, fsImpl), path.basename(value));
  }
}

function parseStat(raw) {
  // comm may contain spaces and parentheses; field 22 is starttime.
  const fields = raw.slice(raw.lastIndexOf(')') + 2).trim().split(/\s+/u);
  if (!/^\d+$/u.test(fields[19] || '')) throw new Error('Cannot prove process start ticks');
  return { state: fields[0], startTicks: fields[19] };
}

function readLinuxIdentity(pid, { fsImpl = fs, canonicalPathImpl = canonicalPath } = {}) {
  if (!Number.isInteger(pid) || pid <= 0) throw new Error('Invalid candidate PID');
  const base = `/proc/${pid}`;
  try {
    const before = parseStat(fsImpl.readFileSync(`${base}/stat`, 'utf8'));
    if (before.state === 'Z' || before.state === 'X') return null;
    const exe = canonicalPathImpl(fsImpl.readlinkSync(`${base}/exe`).replace(/ \(deleted\)$/u, ''));
    const cwd = canonicalPathImpl(fsImpl.readlinkSync(`${base}/cwd`));
    const after = parseStat(fsImpl.readFileSync(`${base}/stat`, 'utf8'));
    if (before.startTicks !== after.startTicks) throw new Error(`PID ${pid} identity changed during inspection`);
    if (after.state === 'Z' || after.state === 'X') return null;
    return { pid, startTicks: before.startTicks, exe, cwd };
  } catch (error) {
    if (error.code === 'ENOENT' || error.code === 'ESRCH') {
      // exe/cwd can disappear during exit. A surviving non-zombie stat is
      // ambiguous (including inaccessible/deleted cwd), not proof of absence.
      try {
        const stat = parseStat(fsImpl.readFileSync(`${base}/stat`, 'utf8'));
        if (stat.state === 'Z' || stat.state === 'X') return null;
      } catch (statError) {
        if (statError.code === 'ENOENT' || statError.code === 'ESRCH') return null;
        throw statError;
      }
    }
    throw error;
  }
}

function readPidRecord(pidFile) {
  try {
    return JSON.parse(fs.readFileSync(pidFile, 'utf8'));
  } catch (error) {
    if (error.code === 'ENOENT' || error instanceof SyntaxError) return null;
    throw error;
  }
}

function portOpen(host, port) {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host, port });
    let done = false;
    const finish = (value) => {
      if (done) return;
      done = true;
      socket.destroy();
      resolve(value);
    };
    socket.setTimeout(300);
    socket.once('connect', () => finish(true));
    socket.once('timeout', () => finish(true)); // uncertain listener: fail closed
    socket.once('error', (error) => finish(error.code !== 'ECONNREFUSED'));
  });
}

function sameIdentity(a, b) {
  return a.pid === b.pid && a.startTicks === b.startTicks && a.exe === b.exe && a.cwd === b.cwd;
}

// This policy belongs to cache deletion only. dev-up's explicit lifecycle/
// takeover behavior remains separate, including its non-Linux compatibility.
async function stopOwnedBackend(service, {
  platform = process.platform,
  readPidRecordImpl = readPidRecord,
  removePidRecordImpl = (file) => fs.rmSync(file, { force: true }),
  canonicalPathImpl = canonicalPath,
  readIdentityImpl = readLinuxIdentity,
  listPortOccupantPidsImpl = (...args) => require('../dev-up/process.js').listPortOccupantPids(...args),
  isPortOpenImpl = portOpen,
  signalPidImpl = (pid, signal) => process.kill(pid, signal),
  sleepImpl = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  nowImpl = Date.now,
  timeoutMs = 5000,
  pollMs = 100,
  stopServiceImpl = (...args) => require('../dev-up/process.js').stopService(...args),
  logImpl = () => {},
} = {}) {
  if (platform !== 'linux') {
    await stopServiceImpl(service, { logImpl, platform });
    return;
  }

  const cwd = canonicalPathImpl(service.cwd);
  const target = canonicalPathImpl(path.join(service.cwd, 'target'));
  function inside(parent, child) {
    const relative = path.relative(parent, child);
    return relative !== '' && relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative);
  }
  const executables = new Set(['debug', 'release'].map((profile) =>
    canonicalPathImpl(path.join(service.cwd, 'target', profile, 'api-server'))));
  if (!inside(cwd, target) || [...executables].some((exe) => !inside(target, exe))) {
    throw new Error('Cannot prove ownership of API executable outside worktree target; refusing cache cleanup');
  }
  const record = readPidRecordImpl(service.pidFile);
  const host = service.probeHost || service.host || '127.0.0.1';

  async function candidates() {
    const pids = listPortOccupantPidsImpl(service.port);
    if (await isPortOpenImpl(host, service.port) && pids.length === 0) {
      throw new Error('Cannot prove ownership of configured API listener; refusing cache cleanup');
    }
    return pids;
  }

  function prove(pid) {
    const identity = readIdentityImpl(pid);
    if (!identity) return null;
    if (!identity.startTicks || identity.pid !== pid || identity.cwd !== cwd || !executables.has(identity.exe)) {
      throw new Error(`Cannot prove ownership of API candidate PID ${pid}; refusing cache cleanup`);
    }
    return identity;
  }

  // Inspect every live candidate before the first signal: a foreign configured
  // port must not cause even a valid recorded API to be stopped incidentally.
  const pids = new Set(await candidates());
  if (Number.isInteger(record?.pid) && record.pid > 0) pids.add(record.pid);
  const identities = [...pids].map(prove).filter(Boolean);

  function current(identity) {
    const value = readIdentityImpl(identity.pid);
    if (!value) return null;
    if (!sameIdentity(identity, value)) {
      throw new Error(`PID ${identity.pid} identity changed; refusing cache cleanup`);
    }
    return value;
  }
  async function waitStopped(identity) {
    const deadline = nowImpl() + timeoutMs;
    while (current(identity)) {
      if (nowImpl() >= deadline) return false;
      await sleepImpl(pollMs);
    }
    return true;
  }
  function signal(identity, value) {
    if (!current(identity)) return;
    try {
      // Positive PID only; never trust ownerPid or signal a process group.
      signalPidImpl(identity.pid, value);
    } catch (error) {
      if (error.code !== 'ESRCH') throw error;
      if (current(identity)) throw error;
    }
  }

  for (const identity of identities) {
    signal(identity, 'SIGTERM');
    if (!(await waitStopped(identity))) {
      signal(identity, 'SIGKILL');
      if (!(await waitStopped(identity))) throw new Error(`Owned API PID ${identity.pid} still running; refusing cache cleanup`);
    }
  }

  // A newly appeared listener is a failure, not permission for port takeover.
  const remaining = await candidates();
  for (const pid of remaining) {
    if (prove(pid)) throw new Error(`API listener PID ${pid} appeared during stop; refusing cache cleanup`);
  }
  if (await isPortOpenImpl(host, service.port)) throw new Error('API listener still open; refusing cache cleanup');
  for (const identity of identities) {
    if (current(identity)) throw new Error(`Owned API PID ${identity.pid} still running; refusing cache cleanup`);
  }
  removePidRecordImpl(service.pidFile);
  logImpl('api-server ownership verified; stopped before cache cleanup');
}

module.exports = { canonicalPath, readLinuxIdentity, stopOwnedBackend };
