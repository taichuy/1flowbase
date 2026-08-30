const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('path');

const {
  CARGO_COLD_STARTUP_TIMEOUT_MS,
  DEFAULT_STARTUP_TIMEOUT_MS,
  buildDevDatabaseMaintenanceHintLines,
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
  waitForServicePort,
} = require('../core.js');

test('AC-001 dev-up runtime messages remain ASCII-only for cross-platform terminals', () => {
  const devUpDir = path.resolve(__dirname, '..');
  const runtimeFiles = [
    path.resolve(devUpDir, '..', 'dev-up.js'),
    ...fs
      .readdirSync(devUpDir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.endsWith('.js'))
      .map((entry) => path.join(devUpDir, entry.name)),
  ];

  for (const filePath of runtimeFiles) {
    const source = fs.readFileSync(filePath, 'utf8');
    assert.doesNotMatch(source, /[^\x00-\x7F]/u, filePath);
  }
});

test('parseCliArgs defaults to full start', () => {
  assert.deepEqual(parseCliArgs([]), {
    action: 'start',
    scope: 'all',
    skipDocker: false,
    help: false,
  });
});

test('parseCliArgs supports backend restart without docker', () => {
  assert.deepEqual(parseCliArgs(['restart', '--backend-only', '--skip-docker']), {
    action: 'restart',
    scope: 'backend',
    skipDocker: true,
    help: false,
  });
});

test('shouldManageDocker skips docker for frontend-only runs', () => {
  assert.equal(
    shouldManageDocker({
      scope: 'frontend',
      skipDocker: false,
    }),
    false
  );
});

test('selectServiceKeys maps scopes to managed services', () => {
  assert.deepEqual(selectServiceKeys('all'), ['web', 'api-server']);
  assert.deepEqual(selectServiceKeys('frontend'), ['web']);
  assert.deepEqual(selectServiceKeys('backend'), ['api-server']);
});

test('dev-up suggests manual development database maintenance for backend starts only', () => {
  assert.equal(shouldShowDevDatabaseMaintenanceHint(parseCliArgs([])), true);
  assert.equal(shouldShowDevDatabaseMaintenanceHint(parseCliArgs(['restart', '--backend-only'])), true);
  assert.equal(shouldShowDevDatabaseMaintenanceHint(parseCliArgs(['--frontend-only'])), false);
  assert.equal(shouldShowDevDatabaseMaintenanceHint(parseCliArgs(['stop'])), false);
  assert.equal(shouldShowDevDatabaseMaintenanceHint(parseCliArgs(['status'])), false);

  const hint = buildDevDatabaseMaintenanceHintLines().join('\n');
  assert.match(hint, /Development databases are not cleaned automatically by dev-up/u);
  assert.match(hint, /test-schemas --dry-run --older-than 3d --keep 20/u);
  assert.match(hint, /backups --dry-run --keep 1 --older-than 7d/u);
  assert.match(hint, /postgres\.empty-\* \/ postgres\.backup-\*/u);
});

test('getServiceDefinitions uses repo default ports and explicit backend binaries', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-defaults-'));
  const services = getServiceDefinitions(tempRepoRoot);

  assert.equal(services.web.port, 3100);
  assert.equal(services['api-server'].port, 7800);
  assert.equal(services.web.bindHost, '0.0.0.0');
  assert.equal(services.web.probeHost, '127.0.0.1');
  assert.equal(services['api-server'].bindHost, '0.0.0.0');
  assert.equal(services['api-server'].probeHost, '127.0.0.1');
  assert.deepEqual(services.web.args, ['--filter', '@1flowbase/web', 'dev']);
  assert.deepEqual(services['api-server'].args, ['run', '-p', 'api-server', '--bin', 'api-server']);
  assert.deepEqual(services.web.readinessProbe, {
    path: '/__1flowbase_dev_ready',
    expectedJson: {
      schema_version: '1flowbase.dev-runtime-readiness/v1',
      state: 'Ready',
    },
  });
  assert.deepEqual(services['api-server'].readinessProbe, {
    path: '/health',
    expectedJson: {
      service: 'api-server',
      status: 'ok',
    },
  });
});

test('AC-001 probeHttpReadiness rejects a reachable service whose health payload is wrong', async () => {
  const server = http.createServer((_request, response) => {
    response.writeHead(200, { 'content-type': 'application/json' });
    response.end(JSON.stringify({ service: 'api-server', status: 'starting' }));
  });

  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();

  try {
    const readiness = await probeHttpReadiness({
      label: 'api-server',
      probeHost: '127.0.0.1',
      port: address.port,
      readinessProbe: {
        path: '/health',
        expectedJson: {
          service: 'api-server',
          status: 'ok',
        },
      },
    });

    assert.deepEqual(readiness, {
      ready: false,
      reason: 'HTTP GET /health returned unexpected JSON field status="starting"',
    });
  } finally {
    await new Promise((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())));
  }
});

test('AC-001 probeHttpReadiness reports a non-object health payload instead of throwing', async () => {
  const server = http.createServer((_request, response) => {
    response.writeHead(200, { 'content-type': 'application/json' });
    response.end('null');
  });

  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();

  try {
    const readiness = await probeHttpReadiness({
      label: 'api-server',
      probeHost: '127.0.0.1',
      port: address.port,
      readinessProbe: {
        path: '/health',
        expectedJson: {
          service: 'api-server',
          status: 'ok',
        },
      },
    });

    assert.deepEqual(readiness, {
      ready: false,
      reason: 'HTTP GET /health returned JSON that is not an object',
    });
  } finally {
    await new Promise((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())));
  }
});

test('AC-002 startService reports a failed readiness probe with log tail and cleans up the child', async () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-readiness-failure-'));
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
    readinessProbe: { path: '/health' },
    logFile: path.join(tempRoot, 'api-server.log'),
    pidFile: path.join(tempRoot, 'api-server.json'),
  };
  const stopped = [];

  await assert.rejects(
    startService(service, {
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
      spawnImpl() {
        return {
          pid: 4242,
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
      async waitForServiceReadinessImpl() {
        return {
          ready: false,
          reason: 'HTTP GET /health timed out after 1000ms',
        };
      },
      async stopServiceImpl(stoppedService) {
        stopped.push(stoppedService.label);
      },
      listPortOccupantPidsImpl() {
        return [];
      },
      takeOverPortOwnership: true,
    }),
    (error) => {
      assert.match(error.message, /api-server readiness failed: HTTP GET \/health timed out after 1000ms/u);
      assert.match(error.message, new RegExp(`log: ${service.logFile.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`, 'u'));
      assert.match(error.message, /last log lines:\n\(log is empty\)/u);
      return true;
    }
  );

  assert.deepEqual(stopped, ['api-server']);
});

test('getServiceDefinitions reads frontend env from web app env file', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-ports-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const webAppDir = path.join(tempRepoRoot, 'web', 'app');
  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.mkdirSync(webAppDir, { recursive: true });
  fs.writeFileSync(
    path.join(apiServerDir, '.env'),
    [
      'API_SERVER_ADDR=0.0.0.0:7900',
      'VITE_API_PROXY_TARGET=http://127.0.0.1:7900',
    ].join('\n')
  );
  fs.writeFileSync(
    path.join(webAppDir, '.env'),
    [
      'VITE_DEV_SERVER_PORT=3200',
      'VITE_API_PROXY_TARGET=https://1flowbase.example.test',
    ].join('\n')
  );

  const services = getServiceDefinitions(tempRepoRoot);

  assert.equal(services.web.port, 3200);
  assert.equal(services['api-server'].port, 7900);
  assert.equal(services.web.envFile, path.join(webAppDir, '.env'));
  assert.equal(
    buildServiceEnv(services.web, {}).VITE_API_PROXY_TARGET,
    'https://1flowbase.example.test'
  );
});

test('dev-up seeds a new web env from existing worktree port configuration', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-web-env-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const webAppDir = path.join(tempRepoRoot, 'web', 'app');
  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.mkdirSync(webAppDir, { recursive: true });
  fs.writeFileSync(
    path.join(apiServerDir, '.env'),
    [
      'API_SERVER_ADDR=0.0.0.0:7900',
      'VITE_DEV_SERVER_PORT=3200',
      'VITE_API_PROXY_TARGET=http://127.0.0.1:7900',
    ].join('\n')
  );
  fs.writeFileSync(
    path.join(webAppDir, '.env.example'),
    [
      'VITE_DEV_SERVER_PORT=3100',
      'VITE_API_PROXY_TARGET=http://127.0.0.1:7800',
    ].join('\n')
  );

  const services = getServiceDefinitions(tempRepoRoot);

  assert.equal(services.web.port, 3200);
  assert.equal(ensureServiceEnvFile(services.web), true);
  assert.equal(buildServiceEnv(services.web, {}).VITE_DEV_SERVER_PORT, '3200');
  assert.equal(
    buildServiceEnv(services.web, {}).VITE_API_PROXY_TARGET,
    'http://127.0.0.1:7900'
  );
});

test('getServiceDefinitions gives cargo services extra startup time for cold cargo builds', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const services = getServiceDefinitions(repoRoot);

  assert.equal(services.web.startupTimeoutMs, 60_000);
  assert.equal(services['api-server'].startupTimeoutMs, CARGO_COLD_STARTUP_TIMEOUT_MS);
});

test('getServiceDefinitions leaves frontend pnpm startup interactive', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const services = getServiceDefinitions(repoRoot);

  assert.equal(services.web.envOverrides, undefined);
});

test('waitForServicePort honors per-service startup timeout overrides', async () => {
  const calls = [];

  const ready = await waitForServicePort(
    {
      probeHost: '127.0.0.1',
      port: 7811,
      startupTimeoutMs: 60_000,
    },
    async (host, port, timeoutMs) => {
      calls.push({ host, port, timeoutMs });
      return true;
    }
  );

  assert.equal(ready, true);
  assert.deepEqual(calls, [
    {
      host: '127.0.0.1',
      port: 7811,
      timeoutMs: 60_000,
    },
  ]);
});

test('waitForPortToClose waits until a cleared port stops accepting connections', async () => {
  const probes = [true, true, false];
  const closed = await waitForPortToClose('127.0.0.1', 3100, 1000, async () => probes.shift());

  assert.equal(closed, true);
});

test('parseWindowsNetstatPortOccupants extracts unique listening pids for a port', () => {
  const output = [
    '  Proto  Local Address          Foreign Address        State           PID',
    '  TCP    0.0.0.0:3100           0.0.0.0:0              LISTENING       31856',
    '  TCP    127.0.0.1:3100         127.0.0.1:14248        TIME_WAIT       0',
    '  TCP    127.0.0.1:3100         127.0.0.1:16943        ESTABLISHED     31856',
    '  TCP    [::]:3100              [::]:0                 LISTENING       31856',
    '  TCP    0.0.0.0:7800           0.0.0.0:0              LISTENING       7800',
  ].join('\n');

  assert.deepEqual(parseWindowsNetstatPortOccupants(output, 3100), [31856]);
});

test('listPortOccupantPids uses netstat on Windows', () => {
  const calls = [];
  const occupants = listPortOccupantPids(3100, {
    platform: 'win32',
    runCommandImpl(command, args, options) {
      calls.push({ command, args, captureOutput: options.captureOutput });
      return {
        status: 0,
        stdout: 'TCP    0.0.0.0:3100           0.0.0.0:0              LISTENING       31856',
        stderr: '',
      };
    },
  });

  assert.deepEqual(occupants, [31856]);
  assert.deepEqual(calls, [
    {
      command: 'netstat',
      args: ['-ano'],
      captureOutput: true,
    },
  ]);
});

test('listPortOccupantPids falls back to ss when lsof cannot resolve listeners', () => {
  const calls = [];
  const occupants = listPortOccupantPids(3100, {
    platform: 'linux',
    commandExistsImpl(command) {
      return command === 'lsof' || command === 'ss';
    },
    runCommandImpl(command, args, options) {
      calls.push({ command, args, captureOutput: options.captureOutput });
      if (command === 'lsof') {
        return {
          status: 1,
          stdout: '',
          stderr: '',
        };
      }

      return {
        status: 0,
        stdout: [
          'State  Recv-Q Send-Q Local Address:Port Peer Address:Port Process',
          'LISTEN 0      511          0.0.0.0:3100      0.0.0.0:*     users:(("node",pid=2468,fd=22),("node",pid=1357,fd=23))',
        ].join('\n'),
        stderr: '',
      };
    },
  });

  assert.deepEqual(occupants, [2468, 1357]);
  assert.deepEqual(
    calls.map((call) => call.command),
    ['lsof', 'ss']
  );
});

test('resolveCommandPath prefers Windows command shims that spawn can execute', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-command-path-'));
  fs.writeFileSync(path.join(tempRoot, 'pnpm'), '');
  fs.writeFileSync(path.join(tempRoot, 'pnpm.CMD'), '');

  assert.equal(
    resolveCommandPath('pnpm', {
      platform: 'win32',
      sourceEnv: { PATH: tempRoot },
    }),
    path.join(tempRoot, 'pnpm.cmd')
  );
});

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

test('manageServices treats start as a service takeover', async () => {
  const service = {
    key: 'web',
    label: 'frontend',
  };
  const calls = [];

  await manageServices('start', [service], {
    async startServiceImpl(startedService, options) {
      calls.push({
        key: startedService.key,
        takeOverPortOwnership: options.takeOverPortOwnership,
      });
    },
  });

  assert.deepEqual(calls, [
    {
      key: 'web',
      takeOverPortOwnership: true,
    },
  ]);
});

test('manageDocker restart clears middleware port conflicts before bringing services up', async () => {
  const composeCalls = [];
  const clearCalls = [];

  await manageDocker('/repo-root', 'restart', {
    ensureMiddlewareEnvImpl() {},
    getMiddlewareHostPortsImpl() {
      return [35432];
    },
    async clearPortConflictsImpl(label, ports) {
      clearCalls.push({ label, ports });
    },
    runMiddlewareComposeImpl(_repoRoot, args) {
      composeCalls.push(args);
      return {
        status: 0,
        stdout: '',
        stderr: '',
      };
    },
  });

  assert.deepEqual(clearCalls, [
    {
      label: 'docker middleware',
      ports: [35432],
    },
  ]);
  assert.deepEqual(composeCalls, [['down'], ['up', '-d']]);
});

test('api-server example env files use workspace bootstrap naming', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const developmentExample = fs.readFileSync(
    path.join(repoRoot, 'api', 'apps', 'api-server', '.env.example'),
    'utf8'
  );
  const productionExample = fs.readFileSync(
    path.join(repoRoot, 'api', 'apps', 'api-server', '.env.production.example'),
    'utf8'
  );

  assert.match(developmentExample, /^BOOTSTRAP_WORKSPACE_NAME=/mu);
  assert.doesNotMatch(developmentExample, /^BOOTSTRAP_TEAM_NAME=/mu);
  assert.match(productionExample, /^BOOTSTRAP_WORKSPACE_NAME=/mu);
  assert.doesNotMatch(productionExample, /^BOOTSTRAP_TEAM_NAME=/mu);
});

test('ensureServiceEnvFile seeds api env defaults and buildServiceEnv loads them', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-env-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const envExamplePath = path.join(apiServerDir, '.env.example');

  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.writeFileSync(
    envExamplePath,
    [
      '# api defaults',
      'API_DATABASE_URL=postgres://from-example',
      'BOOTSTRAP_WORKSPACE_NAME=\"1flowbase\"',
    ].join('\n')
  );

  const services = getServiceDefinitions(tempRepoRoot);
  const apiService = services['api-server'];

  assert.equal(fs.existsSync(apiService.envFile), false);
  assert.equal(ensureServiceEnvFile(apiService), true);
  assert.equal(fs.existsSync(apiService.envFile), true);

  const env = buildServiceEnv(apiService, {
    API_DATABASE_URL: 'postgres://from-shell',
    EXTRA_FLAG: 'enabled',
  });

  assert.equal(env.API_DATABASE_URL, 'postgres://from-shell');
  assert.equal(env.BOOTSTRAP_WORKSPACE_NAME, '1flowbase');
  assert.equal(env.EXTRA_FLAG, 'enabled');
});

test('buildServiceEnv applies service env overrides after shell env', () => {
  const env = buildServiceEnv(
    {
      envOverrides: {
        CI: 'true',
      },
    },
    {
      CI: 'false',
      PATH: '/bin',
    }
  );

  assert.equal(env.CI, 'true');
  assert.equal(env.PATH, '/bin');
});

test('ensureServiceEnvFile leaves existing api-server env values untouched even if they use old branding', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-legacy-env-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const envExamplePath = path.join(apiServerDir, '.env.example');
  const envPath = path.join(apiServerDir, '.env');

  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.writeFileSync(
    envExamplePath,
    [
      'API_DATABASE_URL=postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase',
      'API_COOKIE_NAME=flowbase_console_session',
      'BOOTSTRAP_WORKSPACE_NAME=1flowbase',
    ].join('\n')
  );
  fs.writeFileSync(
    envPath,
    [
      'API_DATABASE_URL=postgres://postgres:sevenflows@127.0.0.1:35432/sevenflows',
      'API_COOKIE_NAME=flowse_console_session',
      'BOOTSTRAP_WORKSPACE_NAME=1Flowse',
      'BOOTSTRAP_ROOT_PASSWORD=change-me',
    ].join('\n')
  );

  const services = getServiceDefinitions(tempRepoRoot);
  const apiService = services['api-server'];

  assert.equal(ensureServiceEnvFile(apiService), false);

  const env = buildServiceEnv(apiService, {});

  assert.equal(env.API_DATABASE_URL, 'postgres://postgres:sevenflows@127.0.0.1:35432/sevenflows');
  assert.equal(env.API_COOKIE_NAME, 'flowse_console_session');
  assert.equal(env.BOOTSTRAP_WORKSPACE_NAME, '1Flowse');
  assert.equal(env.BOOTSTRAP_ROOT_PASSWORD, 'change-me');
});

test('resolveComposeCommand falls back to standalone docker-compose v2', () => {
  const command = resolveComposeCommand({
    resetCache: true,
    runCommandImpl(command, args) {
      if (command === 'docker' && args[0] === 'compose') {
        return {
          status: 1,
          stdout: '',
          stderr: 'docker compose plugin missing\n',
        };
      }

      if (command === 'docker-compose') {
        return {
          status: 0,
          stdout: 'Docker Compose version v2.33.1\n',
          stderr: '',
        };
      }

      return {
        status: 1,
        stdout: '',
        stderr: '',
      };
    },
  });

  assert.deepEqual(command, { command: 'docker-compose', baseArgs: [] });
});
