const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('path');

const {
  DEFAULT_STARTUP_TIMEOUT_MS,
  buildDevDatabaseMaintenanceHintLines,
  configurePostgresToolchain,
  getMiddlewareAction,
  parseCliArgs,
  shouldManageDocker,
  shouldShowDevDatabaseMaintenanceHint,
  selectServiceKeys,
  getServiceDefinitions,
  listPortOccupantPids,
  manageDocker,
  startService,
  manageServices,
  ensureServiceEnvFile,
  buildServiceEnv,
  getServicePrestartCommands,
  parseWindowsNetstatPortOccupants,
  probeHttpReadiness,
  resolveCommandPath,
  runServicePrestartCommands,
  resolveComposeCommand,
  stopService,
  waitForPortToClose,
  waitForServiceReadiness,
  waitForServicePort,
} = require('../core.js');

test('startService clears an occupied frontend port before spawning during takeover', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-port-occupied-'));
  const service = {
    key: 'web',
    label: 'frontend',
    cwd: path.join(tempRoot, 'web'),
    command: 'pnpm',
    args: ['--filter', '@1flowbase/web', 'dev'],
    bindHost: '0.0.0.0',
    probeHost: '127.0.0.1',
    port: 3100,
    startupTimeoutMs: DEFAULT_STARTUP_TIMEOUT_MS,
    logFile: path.join(tempRoot, 'web.log'),
    pidFile: path.join(tempRoot, 'web.json'),
  };
  const clearCalls = [];
  const waitForCloseCalls = [];
  let portOccupied = true;
  let spawned = false;
  let recordedPid = null;

  await startService(service, {
    ensureServiceEnvFileImpl() {
      return false;
    },
    requireCommandImpl() {},
    runServicePrestartCommandsImpl() {},
    readPidRecordImpl() {
      return null;
    },
    isProcessAliveImpl() {
      return false;
    },
    async isPortOpenImpl() {
      return portOccupied;
    },
    async clearPortConflictsImpl(label, ports) {
      clearCalls.push({ label, ports });
      portOccupied = false;
    },
    async waitForPortToCloseImpl(host, port, timeoutMs) {
      waitForCloseCalls.push({ host, port, timeoutMs });
      return true;
    },
    logImpl() {},
    spawnImpl() {
      spawned = true;
      return {
        pid: 4242,
        unref() {},
      };
    },
    buildServiceEnvImpl() {
      return {};
    },
    writePidRecordImpl(_service, pid) {
      recordedPid = pid;
    },
    listPortOccupantPidsImpl() {
      return [];
    },
    async waitForServicePortImpl() {
      return true;
    },
    takeOverPortOwnership: true,
  });

  assert.deepEqual(clearCalls, [
    {
      label: 'frontend',
      ports: [3100],
    },
  ]);
  assert.deepEqual(waitForCloseCalls, [
    {
      host: '127.0.0.1',
      port: 3100,
      timeoutMs: 5000,
    },
  ]);
  assert.equal(spawned, true);
  assert.equal(recordedPid, 4242);
});

test('startService reclaims an occupied service port during restart takeover before spawning', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-port-takeover-'));
  const service = {
    key: 'api-server',
    label: 'api-server',
    cwd: path.join(tempRoot, 'api'),
    command: 'cargo',
    args: ['run', '-p', 'api-server', '--bin', 'api-server'],
    bindHost: '0.0.0.0',
    probeHost: '127.0.0.1',
    port: 7800,
    startupTimeoutMs: DEFAULT_STARTUP_TIMEOUT_MS,
    logFile: path.join(tempRoot, 'api-server.log'),
    pidFile: path.join(tempRoot, 'api-server.json'),
  };
  const clearCalls = [];
  const waitForCloseCalls = [];
  let portOccupied = true;
  let spawned = false;
  let recordedPid = null;

  await startService(service, {
    ensureServiceEnvFileImpl() {
      return false;
    },
    requireCommandImpl() {},
    runServicePrestartCommandsImpl() {},
    readPidRecordImpl() {
      return null;
    },
    isProcessAliveImpl() {
      return false;
    },
    async isPortOpenImpl() {
      return portOccupied;
    },
    async clearPortConflictsImpl(label, ports) {
      clearCalls.push({ label, ports });
      portOccupied = false;
    },
    async waitForPortToCloseImpl(host, port, timeoutMs) {
      waitForCloseCalls.push({ host, port, timeoutMs });
      return true;
    },
    logImpl() {},
    spawnImpl() {
      spawned = true;
      return {
        pid: 4242,
        unref() {},
      };
    },
    buildServiceEnvImpl() {
      return {};
    },
    writePidRecordImpl(_service, pid) {
      recordedPid = pid;
    },
    listPortOccupantPidsImpl() {
      return [];
    },
    async waitForServicePortImpl() {
      return true;
    },
    takeOverPortOwnership: true,
  });

  assert.deepEqual(clearCalls, [
    {
      label: 'api-server',
      ports: [7800],
    },
  ]);
  assert.deepEqual(waitForCloseCalls, [
    {
      host: '127.0.0.1',
      port: 7800,
      timeoutMs: 5000,
    },
  ]);
  assert.equal(spawned, true);
  assert.equal(recordedPid, 4242);
});

test('stopService clears the service port when the pid record is missing', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-stop-orphan-'));
  const service = {
    key: 'web',
    label: 'frontend',
    repoRoot: tempRoot,
    bindHost: '0.0.0.0',
    probeHost: '127.0.0.1',
    port: 3100,
    logFile: path.join(tempRoot, 'web.log'),
    pidFile: path.join(tempRoot, 'web.json'),
  };
  const clearCalls = [];
  const logs = [];
  let portOccupied = true;

  await stopService(service, {
    readPidRecordImpl() {
      return null;
    },
    async isPortOpenImpl() {
      return portOccupied;
    },
    async clearPortConflictsImpl(label, ports) {
      clearCalls.push({ label, ports });
      portOccupied = false;
    },
    async waitForPortToCloseImpl() {
      return true;
    },
    logImpl(message) {
      logs.push(message);
    },
  });

  assert.deepEqual(clearCalls, [
    {
      label: 'frontend',
      ports: [3100],
    },
  ]);
  assert.deepEqual(logs, [
    'frontend has no PID record; clearing port occupants',
    'frontend port occupants cleared',
  ]);
});

test('startService resolves the command path before spawning', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-spawn-command-'));
  const service = {
    key: 'web',
    label: 'frontend',
    cwd: path.join(tempRoot, 'web'),
    command: 'pnpm',
    args: ['--filter', '@1flowbase/web', 'dev'],
    bindHost: '0.0.0.0',
    probeHost: '127.0.0.1',
    port: 3100,
    startupTimeoutMs: DEFAULT_STARTUP_TIMEOUT_MS,
    logFile: path.join(tempRoot, 'web.log'),
    pidFile: path.join(tempRoot, 'web.json'),
  };
  let spawnedCommand = null;
  let spawnedOptions = null;

  await startService(service, {
    ensureServiceEnvFileImpl() {
      return false;
    },
    requireCommandImpl() {},
    runServicePrestartCommandsImpl() {},
    readPidRecordImpl() {
      return null;
    },
    isProcessAliveImpl() {
      return false;
    },
    async isPortOpenImpl() {
      return false;
    },
    logImpl() {},
    spawnImpl(command, _args, options) {
      spawnedCommand = command;
      spawnedOptions = options;
      return {
        pid: 4244,
        unref() {},
      };
    },
    buildServiceEnvImpl() {
      return {};
    },
    resolveCommandPathImpl() {
      return 'C:\\tools\\pnpm.cmd';
    },
    writePidRecordImpl() {},
    async waitForServicePortImpl() {
      return true;
    },
    platform: 'win32',
    takeOverPortOwnership: true,
  });

  assert.equal(spawnedCommand, 'C:\\tools\\pnpm.cmd');
  assert.equal(spawnedOptions.shell, true);
  assert.equal(spawnedOptions.detached, false);
});

test('startService truncates stale service logs before spawning', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-log-truncate-'));
  const service = {
    key: 'web',
    label: 'frontend',
    cwd: path.join(tempRoot, 'web'),
    command: 'pnpm',
    args: ['--filter', '@1flowbase/web', 'dev'],
    bindHost: '0.0.0.0',
    probeHost: '127.0.0.1',
    port: 3100,
    startupTimeoutMs: DEFAULT_STARTUP_TIMEOUT_MS,
    logFile: path.join(tempRoot, 'web.log'),
    pidFile: path.join(tempRoot, 'web.json'),
  };

  fs.writeFileSync(service.logFile, 'old vite ready line\n', 'utf8');

  await startService(service, {
    ensureServiceEnvFileImpl() {
      return false;
    },
    requireCommandImpl() {},
    runServicePrestartCommandsImpl() {},
    readPidRecordImpl() {
      return null;
    },
    isProcessAliveImpl() {
      return false;
    },
    async isPortOpenImpl() {
      return false;
    },
    logImpl() {},
    spawnImpl() {
      return {
        pid: 4245,
        unref() {},
      };
    },
    buildServiceEnvImpl() {
      return {};
    },
    writePidRecordImpl() {},
    async waitForServicePortImpl() {
      return true;
    },
    listPortOccupantPidsImpl() {
      return [];
    },
    takeOverPortOwnership: true,
  });

  assert.equal(fs.readFileSync(service.logFile, 'utf8'), '');
});

test('startService records the listener pid when it differs from the spawned shell pid', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-listener-pid-'));
  const service = {
    key: 'web',
    label: 'frontend',
    cwd: path.join(tempRoot, 'web'),
    command: 'pnpm',
    args: ['--filter', '@1flowbase/web', 'dev'],
    bindHost: '0.0.0.0',
    probeHost: '127.0.0.1',
    port: 3100,
    startupTimeoutMs: DEFAULT_STARTUP_TIMEOUT_MS,
    logFile: path.join(tempRoot, 'web.log'),
    pidFile: path.join(tempRoot, 'web.json'),
  };
  const recordedPids = [];

  await startService(service, {
    ensureServiceEnvFileImpl() {
      return false;
    },
    requireCommandImpl() {},
    runServicePrestartCommandsImpl() {},
    readPidRecordImpl() {
      return null;
    },
    isProcessAliveImpl() {
      return false;
    },
    async isPortOpenImpl() {
      return false;
    },
    logImpl() {},
    spawnImpl() {
      return {
        pid: 1111,
        unref() {},
      };
    },
    buildServiceEnvImpl() {
      return {};
    },
    resolveCommandPathImpl() {
      return 'C:\\tools\\pnpm.cmd';
    },
    writePidRecordImpl(_service, pid) {
      recordedPids.push(pid);
    },
    async waitForServicePortImpl() {
      return true;
    },
    listPortOccupantPidsImpl() {
      return [2222];
    },
    platform: 'win32',
    takeOverPortOwnership: true,
  });

  assert.deepEqual(recordedPids, [1111, 2222]);
});

test('startService restarts a running managed service when takeover is requested', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-managed-takeover-'));
  const service = {
    key: 'web',
    label: 'frontend',
    cwd: path.join(tempRoot, 'web'),
    command: 'pnpm',
    args: ['--filter', '@1flowbase/web', 'dev'],
    bindHost: '0.0.0.0',
    probeHost: '127.0.0.1',
    port: 3100,
    startupTimeoutMs: DEFAULT_STARTUP_TIMEOUT_MS,
    logFile: path.join(tempRoot, 'web.log'),
    pidFile: path.join(tempRoot, 'web.json'),
  };
  const stopCalls = [];
  let portOpen = true;
  let spawned = false;
  let recordedPid = null;

  await startService(service, {
    ensureServiceEnvFileImpl() {
      return false;
    },
    requireCommandImpl() {},
    runServicePrestartCommandsImpl() {},
    readPidRecordImpl() {
      return { pid: 3100 };
    },
    isProcessAliveImpl() {
      return true;
    },
    async isPortOpenImpl() {
      return portOpen;
    },
    async stopServiceImpl(stoppedService) {
      stopCalls.push(stoppedService.key);
      portOpen = false;
    },
    logImpl() {},
    spawnImpl() {
      spawned = true;
      return {
        pid: 4243,
        unref() {},
      };
    },
    buildServiceEnvImpl() {
      return {};
    },
    writePidRecordImpl(_service, pid) {
      recordedPid = pid;
    },
    listPortOccupantPidsImpl() {
      return [];
    },
    async waitForServicePortImpl() {
      return true;
    },
    takeOverPortOwnership: true,
  });

  assert.deepEqual(stopCalls, ['web']);
  assert.equal(spawned, true);
  assert.equal(recordedPid, 4243);
});

