const fs = require('node:fs');
const net = require('node:net');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');

const { log } = require('./cli.js');
const {
  buildServiceEnv,
  commandExists,
  ensureServiceEnvFile,
  requireCommand,
  resolveCommandPath,
} = require('./env.js');
const { runServicePrestartCommands } = require('./postgres-reset.js');
const { DEFAULT_STARTUP_TIMEOUT_MS } = require('./services.js');

function readPidRecord(pidFile) {
  if (!fs.existsSync(pidFile)) {
    return null;
  }

  try {
    const raw = fs.readFileSync(pidFile, 'utf8');
    return JSON.parse(raw);
  } catch (_error) {
    return null;
  }
}

function getProbeHost(service) {
  return service.probeHost || service.host;
}

function getBindHost(service) {
  return service.bindHost || service.host;
}

function getStartupTimeoutMs(service) {
  if (!service || !Number.isFinite(service.startupTimeoutMs) || service.startupTimeoutMs <= 0) {
    return DEFAULT_STARTUP_TIMEOUT_MS;
  }

  return service.startupTimeoutMs;
}

function writePidRecord(service, pid) {
  fs.writeFileSync(
    service.pidFile,
    JSON.stringify(
      {
        pid,
        command: service.command,
        args: service.args,
        port: service.port,
        startedAt: new Date().toISOString(),
      },
      null,
      2
    )
  );
}

function removePidRecord(pidFile) {
  if (fs.existsSync(pidFile)) {
    fs.unlinkSync(pidFile);
  }
}

function isProcessAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) {
    return false;
  }

  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error.code === 'ESRCH') {
      return false;
    }

    throw error;
  }
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function isPortOpen(host, port, timeoutMs = 300) {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host, port });

    let settled = false;
    const finish = (value) => {
      if (!settled) {
        settled = true;
        socket.destroy();
        resolve(value);
      }
    };

    socket.setTimeout(timeoutMs);
    socket.on('connect', () => finish(true));
    socket.on('timeout', () => finish(false));
    socket.on('error', () => finish(false));
  });
}

async function waitForPort(host, port, timeoutMs = 15000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (await isPortOpen(host, port)) {
      return true;
    }

    await sleep(250);
  }

  return false;
}

async function waitForPortToClose(host, port, timeoutMs = 5000, isPortOpenImpl = isPortOpen) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (!(await isPortOpenImpl(host, port))) {
      return true;
    }

    await sleep(200);
  }

  return !(await isPortOpenImpl(host, port));
}

function waitForServicePort(service, waitForPortImpl = waitForPort) {
  return waitForPortImpl(getProbeHost(service), service.port, getStartupTimeoutMs(service));
}

function runCommand(command, args, options = {}) {
  const resolvedCommand = resolveCommandPath(command) || command;
  return spawnSync(resolvedCommand, args, {
    cwd: options.cwd || process.cwd(),
    env: options.env || process.env,
    encoding: 'utf8',
    maxBuffer: options.maxBuffer || 8 * 1024 * 1024,
    stdio: options.captureOutput ? ['ignore', 'pipe', 'pipe'] : 'inherit',
  });
}

function parseWindowsNetstatPortOccupants(output, port) {
  if (!Number.isInteger(port) || port <= 0) {
    return [];
  }

  const occupants = new Set();
  const portPattern = new RegExp(`^(?:\\[::\\]|\\S+):${port}$`, 'u');

  for (const line of String(output || '').split(/\r?\n/)) {
    const columns = line.trim().split(/\s+/u);
    if (columns.length < 5 || columns[0].toUpperCase() !== 'TCP') {
      continue;
    }

    const [, localAddress, , state, pid] = columns;
    if (state.toUpperCase() !== 'LISTENING' || !portPattern.test(localAddress)) {
      continue;
    }

    const parsedPid = Number.parseInt(pid, 10);
    if (Number.isInteger(parsedPid) && parsedPid > 0) {
      occupants.add(parsedPid);
    }
  }

  return [...occupants];
}

function parsePidList(output) {
  return String(output || '')
    .split(/\s+/u)
    .map((value) => Number.parseInt(value.trim(), 10))
    .filter((value) => Number.isInteger(value) && value > 0);
}

function parseSsPortOccupants(output) {
  const occupants = new Set();

  for (const line of String(output || '').split(/\r?\n/u)) {
    const pidPattern = /\bpid=(\d+)\b/gu;
    let match = pidPattern.exec(line);
    while (match) {
      const parsedPid = Number.parseInt(match[1], 10);
      if (Number.isInteger(parsedPid) && parsedPid > 0) {
        occupants.add(parsedPid);
      }
      match = pidPattern.exec(line);
    }
  }

  return [...occupants];
}

function uniquePids(pids) {
  return [...new Set(pids.filter((pid) => Number.isInteger(pid) && pid > 0))];
}

function listPortOccupantPids(
  port,
  {
    platform = process.platform,
    runCommandImpl = runCommand,
    commandExistsImpl = commandExists,
  } = {}
) {
  if (!Number.isInteger(port) || port <= 0) {
    return [];
  }

  if (platform === 'win32') {
    const result = runCommandImpl('netstat', ['-ano'], {
      captureOutput: true,
    });
    if (result.error || result.status !== 0) {
      return [];
    }

    return parseWindowsNetstatPortOccupants(result.stdout, port);
  }

  if (commandExistsImpl('lsof')) {
    const result = runCommandImpl('lsof', ['-t', `-iTCP:${port}`, '-sTCP:LISTEN', '-P', '-n'], {
      captureOutput: true,
    });
    const occupants = result.error || result.status !== 0 ? [] : parsePidList(result.stdout);
    if (occupants.length > 0) {
      return uniquePids(occupants);
    }
  }

  if (commandExistsImpl('ss')) {
    const result = runCommandImpl('ss', ['-ltnp', 'sport', '=', `:${port}`], {
      captureOutput: true,
    });
    const occupants = result.error || result.status !== 0 ? [] : parseSsPortOccupants(result.stdout);
    if (occupants.length > 0) {
      return uniquePids(occupants);
    }
  }

  if (commandExistsImpl('fuser')) {
    const result = runCommandImpl('fuser', ['-n', 'tcp', String(port)], {
      captureOutput: true,
    });
    const occupants = result.error || result.status !== 0 ? [] : parsePidList(result.stdout);
    if (occupants.length > 0) {
      return uniquePids(occupants);
    }
  }

  return [];
}

function getProcessGroupId(pid) {
  if (!Number.isInteger(pid) || pid <= 0 || !commandExists('ps')) {
    return pid;
  }

  const result = runCommand('ps', ['-o', 'pgid=', '-p', String(pid)], {
    captureOutput: true,
  });
  if (result.error || result.status !== 0) {
    return pid;
  }

  const groupId = Number.parseInt(String(result.stdout || '').trim(), 10);
  return Number.isInteger(groupId) && groupId > 0 ? groupId : pid;
}

function signalProcess(pid, signal) {
  try {
    if (process.platform !== 'win32') {
      process.kill(-getProcessGroupId(pid), signal);
      return;
    }
  } catch (error) {
    if (error.code !== 'ESRCH') {
      try {
        process.kill(pid, signal);
        return;
      } catch (innerError) {
        if (innerError.code !== 'ESRCH') {
          throw innerError;
        }
      }
    } else {
      return;
    }
  }

  try {
    process.kill(pid, signal);
  } catch (error) {
    if (error.code !== 'ESRCH') {
      throw error;
    }
  }
}

async function waitForProcessExit(pid, timeoutMs = 5000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (!isProcessAlive(pid)) {
      return true;
    }

    await sleep(200);
  }

  return !isProcessAlive(pid);
}

async function clearPortConflicts(
  label,
  ports,
  {
    listPortOccupantPidsImpl = listPortOccupantPids,
    signalProcessImpl = signalProcess,
    waitForProcessExitImpl = waitForProcessExit,
    logImpl = log,
  } = {}
) {
  const normalizedPorts = [...new Set(ports.filter((port) => Number.isInteger(port) && port > 0))];

  for (const port of normalizedPorts) {
    const occupants = listPortOccupantPidsImpl(port);
    if (occupants.length === 0) {
      continue;
    }

    logImpl(`${label} detected occupied port ${port}; terminating pid=${occupants.join(',')}`);

    for (const pid of occupants) {
      signalProcessImpl(pid, 'SIGTERM');
    }

    for (const pid of occupants) {
      const exited = await waitForProcessExitImpl(pid);
      if (exited) {
        continue;
      }

      signalProcessImpl(pid, 'SIGKILL');
      await waitForProcessExitImpl(pid, 2000);
    }
  }
}

async function startService(
  service,
  {
    ensureServiceEnvFileImpl = ensureServiceEnvFile,
    requireCommandImpl = requireCommand,
    runServicePrestartCommandsImpl = runServicePrestartCommands,
    readPidRecordImpl = readPidRecord,
    isProcessAliveImpl = isProcessAlive,
    isPortOpenImpl = isPortOpen,
    stopServiceImpl = stopService,
    spawnImpl = spawn,
    buildServiceEnvImpl = buildServiceEnv,
    listPortOccupantPidsImpl = listPortOccupantPids,
    platform = process.platform,
    resolveCommandPathImpl = resolveCommandPath,
    writePidRecordImpl = writePidRecord,
    waitForServicePortImpl = waitForServicePort,
    waitForPortToCloseImpl = waitForPortToClose,
    clearPortConflictsImpl = clearPortConflicts,
    logImpl = log,
    takeOverPortOwnership = false,
  } = {}
) {
  ensureServiceEnvFileImpl(service);
  requireCommandImpl(service.command);

  const pidRecord = readPidRecordImpl(service.pidFile);
  if (pidRecord && isProcessAliveImpl(pidRecord.pid)) {
    if (await isPortOpenImpl(getProbeHost(service), service.port)) {
      if (!takeOverPortOwnership) {
        logImpl(`${service.label} is already running; skipping start`);
        return;
      }

      logImpl(`${service.label} is already running; restarting`);
    }

    await stopServiceImpl(service);
  }

  runServicePrestartCommandsImpl(service);

  if (await isPortOpenImpl(getProbeHost(service), service.port) && takeOverPortOwnership) {
    await clearPortConflictsImpl(service.label, [service.port]);
    await waitForPortToCloseImpl(getProbeHost(service), service.port, 5000, isPortOpenImpl);
  }

  if (await isPortOpenImpl(getProbeHost(service), service.port)) {
    throw new Error(`${service.label} failed to start because port ${service.port} is occupied`);
  }

  const outputFd = fs.openSync(service.logFile, 'w');
  const child = spawnImpl(resolveCommandPathImpl(service.command) || service.command, service.args, {
    cwd: service.cwd,
    env: buildServiceEnvImpl(service),
    detached: platform !== 'win32',
    shell: platform === 'win32',
    stdio: ['ignore', outputFd, outputFd],
  });

  fs.closeSync(outputFd);
  child.unref();
  writePidRecordImpl(service, child.pid);

  const ready = await waitForServicePortImpl(service);
  if (!ready) {
    await stopServiceImpl(service);
    throw new Error(`${service.label} startup timed out; see log: ${service.logFile}`);
  }

  const listenerPids = listPortOccupantPidsImpl(service.port);
  if (listenerPids.length > 0 && listenerPids[0] !== child.pid) {
    writePidRecordImpl(service, listenerPids[0]);
  }

  logImpl(`${service.label} started; listening on ${getBindHost(service)}:${service.port}`);
}

async function clearServicePortOccupants(
  service,
  {
    isPortOpenImpl = isPortOpen,
    waitForPortToCloseImpl = waitForPortToClose,
    clearPortConflictsImpl = clearPortConflicts,
    logImpl = log,
  } = {}
) {
  await clearPortConflictsImpl(service.label, [service.port]);
  const closed = await waitForPortToCloseImpl(getProbeHost(service), service.port, 5000, isPortOpenImpl);
  if (!closed && (await isPortOpenImpl(getProbeHost(service), service.port))) {
    throw new Error(`${service.label} failed to clear port ${service.port}; it is still occupied`);
  }

  logImpl(`${service.label} port occupants cleared`);
}

async function stopService(
  service,
  {
    readPidRecordImpl = readPidRecord,
    isProcessAliveImpl = isProcessAlive,
    signalProcessImpl = signalProcess,
    waitForProcessExitImpl = waitForProcessExit,
    removePidRecordImpl = removePidRecord,
    isPortOpenImpl = isPortOpen,
    waitForPortToCloseImpl = waitForPortToClose,
    clearPortConflictsImpl = clearPortConflicts,
    logImpl = log,
  } = {}
) {
  const pidRecord = readPidRecordImpl(service.pidFile);
  if (!pidRecord) {
    if (await isPortOpenImpl(getProbeHost(service), service.port)) {
      logImpl(`${service.label} has no PID record; clearing port occupants`);
      await clearServicePortOccupants(service, {
        isPortOpenImpl,
        waitForPortToCloseImpl,
        clearPortConflictsImpl,
        logImpl,
      });
      return;
    }

    logImpl(`${service.label} has no PID record; skipping stop`);
    return;
  }

  if (!isProcessAliveImpl(pidRecord.pid)) {
    removePidRecordImpl(service.pidFile);
    if (await isPortOpenImpl(getProbeHost(service), service.port)) {
      logImpl(`${service.label} has a stale process record; clearing port occupants`);
      await clearServicePortOccupants(service, {
        isPortOpenImpl,
        waitForPortToCloseImpl,
        clearPortConflictsImpl,
        logImpl,
      });
      return;
    }

    logImpl(`${service.label} stale process record removed`);
    return;
  }

  signalProcessImpl(pidRecord.pid, 'SIGTERM');
  const exited = await waitForProcessExitImpl(pidRecord.pid);
  if (!exited) {
    signalProcessImpl(pidRecord.pid, 'SIGKILL');
    await waitForProcessExitImpl(pidRecord.pid, 2000);
  }

  removePidRecordImpl(service.pidFile);
  if (await isPortOpenImpl(getProbeHost(service), service.port)) {
    logImpl(`${service.label} recorded process stopped; clearing remaining port occupants`);
    await clearServicePortOccupants(service, {
      isPortOpenImpl,
      waitForPortToCloseImpl,
      clearPortConflictsImpl,
      logImpl,
    });
    return;
  }

  logImpl(`${service.label} stopped`);
}

async function statusService(service) {
  const pidRecord = readPidRecord(service.pidFile);
  const alive = pidRecord ? isProcessAlive(pidRecord.pid) : false;
  const portOpen = await isPortOpen(getProbeHost(service), service.port);
  const status = alive && portOpen ? 'running' : alive ? 'starting' : portOpen ? 'orphaned' : 'stopped';

  log(
    `${service.label}: ${status} | listen=${getBindHost(service)}:${service.port} | probe=${getProbeHost(service)}:${service.port} | pid=${pidRecord ? pidRecord.pid : 'none'} | log=${path.relative(
      service.repoRoot || process.cwd(),
      service.logFile
    )}`
  );
}

async function manageServices(
  action,
  services,
  {
    stopServiceImpl = stopService,
    statusServiceImpl = statusService,
    startServiceImpl = startService,
  } = {}
) {
  if (action === 'stop') {
    for (const service of [...services].reverse()) {
      await stopServiceImpl(service);
    }
    return;
  }

  if (action === 'status') {
    for (const service of services) {
      await statusServiceImpl(service);
    }
    return;
  }

  if (action === 'restart') {
    for (const service of [...services].reverse()) {
      await stopServiceImpl(service);
    }
  }

  for (const service of services) {
    await startServiceImpl(service, {
      takeOverPortOwnership: action === 'start' || action === 'restart',
    });
  }
}

module.exports = {
  listPortOccupantPids,
  manageServices,
  parseWindowsNetstatPortOccupants,
  startService,
  stopService,
  waitForPortToClose,
  waitForServicePort,
};
