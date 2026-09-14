const fs = require('node:fs');
const { spawn, execFile } = require('node:child_process');
const { resolveCommandPath } = require('./env.js');
const { log } = require('./cli.js');

function signalGroup(pid, signal) {
  if (!pid) return;
  if (process.platform === 'win32') {
    // taskkill owns only this phase's process tree.
    return new Promise((resolve, reject) => {
      execFile('taskkill', ['/PID', String(pid), '/T', '/F'], (error) => {
        if (error && error.code !== 128) reject(error);
        else resolve();
      });
    });
  }
  try { process.kill(-pid, signal); } catch (error) {
    if (error.code !== 'ESRCH') throw error;
  }
}

async function stopOwnedProcess(pid) {
  await signalGroup(pid, 'SIGTERM');
  await new Promise((resolve) => setTimeout(resolve, 500));
  await signalGroup(pid, 'SIGKILL');
}

// A phase owns its process group, optional systemd scope, timers and output.
// It never searches for or terminates unrelated port occupants.
async function runPhase(command, args, {
  cwd, env = process.env, signal, logFile, label = command,
  timeoutMs = 120_000, heartbeatMs = 5_000, onLine,
  cleanup, logImpl = log, inheritStdin = false, allowFailure = false,
} = {}) {
  signal?.throwIfAborted();
  const startedAt = Date.now();
  logImpl(`${label}: starting; timeout=${Math.round(timeoutMs / 1000)}s; log=${logFile}`);
  const fd = fs.openSync(logFile, 'w');
  let child;
  try {
    child = spawn(resolveCommandPath(command) || command, args, {
      cwd, env, detached: process.platform !== 'win32',
      shell: process.platform === 'win32' && /\.(cmd|bat)$/i.test(resolveCommandPath(command) || command),
      stdio: [inheritStdin ? 'inherit' : 'ignore', 'pipe', 'pipe'],
    });
  } catch (error) {
    fs.closeSync(fd);
    throw error;
  }
  let stdout = '', stderr = '', pending = '', termination;
  let phaseError;
  const consume = (chunk, isError) => {
    fs.writeSync(fd, chunk);
    if (isError) stderr = (stderr + chunk).slice(-16_384);
    else {
      stdout = (stdout + chunk).slice(-16_384);
      if (onLine) {
        pending += chunk;
        let end;
        while ((end = pending.indexOf('\n')) !== -1) {
          onLine(pending.slice(0, end));
          pending = pending.slice(end + 1);
        }
      }
    }
    if (inheritStdin) (isError ? process.stderr : process.stdout).write(chunk);
  };
  child.stdout.on('data', (chunk) => consume(chunk, false));
  child.stderr.on('data', (chunk) => consume(chunk, true));
  const terminate = (error) => {
    phaseError ||= error;
    termination ||= (async () => {
      // The scope is needed because systemd can move Cargo outside our group.
      let cleanupError;
      try { await cleanup?.(); } catch (error) { cleanupError = error; }
      await stopOwnedProcess(child.pid);
      if (cleanupError) throw cleanupError;
    })();
    // Awaited below; attach immediately to avoid an unhandled rejection.
    termination.catch(() => {});
  };
  const onAbort = () => terminate(signal.reason || new Error(`${label}: cancelled`));
  signal?.addEventListener('abort', onAbort, { once: true });
  const timer = setTimeout(() => terminate(new Error(`${label}: timed out after ${timeoutMs / 1000}s`)), timeoutMs);
  const heartbeat = setInterval(() => {
    logImpl(`${label}: waiting ${Math.round((Date.now() - startedAt) / 1000)}s; log=${logFile}`);
  }, heartbeatMs);
  if (signal?.aborted) onAbort();
  try {
    const result = await new Promise((resolve) => {
      child.once('error', (error) => resolve({ status: null, error }));
      child.once('close', (status, exitSignal) => resolve({ status, signal: exitSignal }));
    });
    const exitError = result.error || (result.status !== 0 ? new Error(`${label}: exited ${result.status ?? result.signal}`) : null);
    if (exitError) terminate(exitError);
    if (termination) await termination;
    if (pending && onLine) onLine(pending);
    if (phaseError && !(allowFailure && phaseError === exitError && !result.error)) throw new Error(`${phaseError.message}\nlog: ${logFile}\n${stderr.slice(-4000)}`, { cause: phaseError });
    logImpl(`${label}: ${result.status === 0 ? 'completed' : 'failed'} in ${Math.round((Date.now() - startedAt) / 1000)}s`);
    return { ...result, stdout, stderr };
  } finally {
    clearTimeout(timer);
    clearInterval(heartbeat);
    signal?.removeEventListener('abort', onAbort);
    fs.closeSync(fd);
  }
}

function acquireStartupLock(pidDir) {
  const lockFile = require('node:path').join(pidDir, 'startup.lock');
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      fs.writeFileSync(lockFile, String(process.pid), { flag: 'wx' });
      return () => fs.unlinkSync(lockFile);
    } catch (error) {
      if (error.code !== 'EEXIST') throw error;
      const pid = Number(fs.readFileSync(lockFile, 'utf8'));
      if (!Number.isInteger(pid) || pid <= 0) throw new Error(`Invalid startup lock: ${lockFile}`);
      try { process.kill(pid, 0); } catch (error) {
        if (error.code !== 'ESRCH') throw error;
        fs.unlinkSync(lockFile);
        continue;
      }
      throw new Error(`Another dev-up operation is active (pid=${pid}); no new work started`);
    }
  }
  throw new Error(`Unable to acquire startup lock: ${lockFile}`);
}

async function withStartupSession(services, pidDir, operation, {
  signalSource = process, stopOwnedProcessImpl = stopOwnedProcess,
} = {}) {
  const release = acquireStartupLock(pidDir);
  const controller = new AbortController();
  const interrupt = () => controller.abort(new Error('dev-up cancelled'));
  signalSource.on('SIGINT', interrupt);
  signalSource.on('SIGTERM', interrupt);
  for (const service of services) service.signal = controller.signal;
  try {
    const result = await operation();
    controller.signal.throwIfAborted();
    return result;
  } catch (error) {
    const failures = [];
    for (const service of [...services].reverse()) {
      if (!service.startedPid) continue;
      try {
        await stopOwnedProcessImpl(service.startedPid);
        fs.rmSync(service.pidFile, { force: true });
      } catch (cleanupError) { failures.push(cleanupError); }
    }
    if (failures.length) {
      throw new AggregateError([error, ...failures], `${error.message}; cleanup failed: ${failures.map((failure) => failure.message).join('; ')}`);
    }
    throw error;
  } finally {
    signalSource.removeListener('SIGINT', interrupt);
    signalSource.removeListener('SIGTERM', interrupt);
    release();
  }
}

module.exports = { runPhase, stopOwnedProcess, acquireStartupLock, withStartupSession };
