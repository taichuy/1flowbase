const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { startService, manageServices, waitForServicePort } = require('../process.js');
const { runPhase, acquireStartupLock } = require('../phases.js');
const { prepareFrontend, dependencyFingerprint } = require('../dependencies.js');
const { buildBackend } = require('../build.js');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'dev-up-startup-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return root;
}

function service(root) {
  return { key: 'api-server', label: 'api-server', repoRoot: root, cwd: root,
    command: 'cargo', args: [], buildBeforeStart: true,
    pidFile: path.join(root, 'api.json'), logFile: path.join(root, 'api.log'),
    port: 7800, probeHost: '127.0.0.1', startupTimeoutMs: 1000 };
}

function launchFixture(s, overrides = {}) {
  return {
    ensureServiceEnvFileImpl() {}, requireCommandImpl() {},
    readPidRecordImpl: () => null, isPortOpenImpl: async () => false,
    runServicePrestartCommandsImpl: async () => {},
    buildBackendImpl: async () => ({ command: '/fixture/api-server', args: [] }),
    buildServiceEnvImpl: () => ({}), resolveCommandPathImpl: (command) => command,
    writePidRecordImpl() {}, waitForServicePortImpl: async () => true,
    waitForServiceReadinessImpl: async () => ({ ready: true }),
    listPortOccupantPidsImpl: () => [], logImpl() {},
    spawnImpl() { return { pid: 123456789, unref() {} }; },
    ...overrides,
  };
}

test('AC-001 healthy service reuse skips preparation, build and spawn', async (t) => {
  const s = service(fixture(t));
  const forbidden = () => assert.fail('reused service must not do startup work');
  await startService(s, launchFixture(s, {
    readPidRecordImpl: () => ({ pid: 42 }), isProcessAliveImpl: () => true,
    isPortOpenImpl: async () => true, probeHttpReadinessImpl: async () => ({ ready: true }),
    requireCommandImpl: forbidden, runServicePrestartCommandsImpl: forbidden,
    buildBackendImpl: forbidden, spawnImpl: forbidden,
  }));
  await assert.rejects(startService(s, launchFixture(s, {
    readPidRecordImpl: () => ({ pid: 42 }), isProcessAliveImpl: () => true,
    isPortOpenImpl: async () => true,
    probeHttpReadinessImpl: async () => ({ ready: false, reason: 'wrong health payload' }),
    spawnImpl: forbidden,
  })), /unhealthy/);
});

test('AC-001 only explicit restart requests takeover', async () => {
  const actions = [];
  for (const action of ['start', 'ensure', 'restart']) {
    await manageServices(action, [{}], {
      stopServiceImpl: async () => {},
      startServiceImpl: async (_service, options) => actions.push([action, options.takeOverPortOwnership]),
    });
  }
  assert.deepEqual(actions, [['start', false], ['ensure', false], ['restart', true]]);
});

test('AC-003 build must complete before launching the exact artifact; failures never run stale binaries', async (t) => {
  const s = service(fixture(t));
  const order = [];
  await startService(s, launchFixture(s, {
    async buildBackendImpl() {
      await Promise.resolve();
      order.push('built');
      return { command: '/custom-target/api-server', args: [] };
    },
    spawnImpl(command, args) {
      order.push([command, args]);
      return { pid: 123456789, unref() {} };
    },
  }));
  assert.deepEqual(order, ['built', ['/custom-target/api-server', []]]);
  await assert.rejects(startService(s, launchFixture(s, {
    buildBackendImpl: async () => { throw new Error('compiler failed'); },
    spawnImpl: () => assert.fail('stale executable must not run'),
  })), /compiler failed/);
});

test('AC-002 dependency receipt reuses installs, invalidates changed inputs and never caches failure', async (t) => {
  const root = fixture(t);
  fs.mkdirSync(path.join(root, 'app'), { recursive: true });
  fs.writeFileSync(path.join(root, 'package.json'), '{"packageManager":"pnpm@10.0.0"}');
  fs.writeFileSync(path.join(root, 'app/package.json'), '{}');
  fs.writeFileSync(path.join(root, 'pnpm-lock.yaml'), 'lockfileVersion: 9');
  const s = { ...service(root), key: 'web', command: 'pnpm' };
  let installs = 0;
  const options = { logImpl() {}, async runPhaseImpl(command, args) {
    assert.equal(command, 'pnpm');
    assert.deepEqual(args, ['install', '--frozen-lockfile']);
    installs += 1;
    fs.mkdirSync(path.join(root, 'node_modules/.pnpm'), { recursive: true });
    fs.mkdirSync(path.join(root, 'app/node_modules'), { recursive: true });
    fs.writeFileSync(path.join(root, 'node_modules/.modules.yaml'), 'installed');
    fs.writeFileSync(path.join(root, 'node_modules/.pnpm/lock.yaml'), 'installed lock');
  } };
  await prepareFrontend(s, options);
  await prepareFrontend(s, options);
  assert.equal(installs, 1);
  fs.writeFileSync(path.join(root, 'app/package.json'), '{"dependencies":{"a":"1"}}');
  await prepareFrontend(s, options);
  assert.equal(installs, 2);
  fs.mkdirSync(path.join(root, 'patches'));
  fs.writeFileSync(path.join(root, 'patches/a.patch'), 'patch');
  await prepareFrontend(s, options);
  assert.equal(installs, 3);
  fs.rmSync(path.join(root, 'app/node_modules'), { recursive: true });
  await assert.rejects(prepareFrontend(s, { ...options, runPhaseImpl: async () => { throw new Error('install failed'); } }), /install failed/);
  assert.equal(fs.existsSync(path.join(root, 'frontend-dependencies.json')), false);
  await prepareFrontend(s, options);
  assert.equal(installs, 4);
  fs.writeFileSync(path.join(root, 'node_modules/.modules.yaml'), 'different installation');
  await prepareFrontend(s, options);
  assert.equal(installs, 5);
  const before = dependencyFingerprint(root);
  fs.writeFileSync(path.join(root, 'app/page.tsx'), 'source edit');
  assert.equal(dependencyFingerprint(root), before);
});

test('AC-003 build calls Cargo directly with no deadline and selects its reported binary', async (t) => {
  const root = fixture(t);
  const binary = path.join(root, 'custom-target/api-server');
  fs.mkdirSync(path.dirname(binary));
  fs.writeFileSync(binary, 'fixture');
  fs.writeFileSync(path.join(root, '.1flowbase.verify.local.json'), JSON.stringify({ backend: { cargoJobs: 1, incremental: true } }));
  const s = { ...service(root), envOverrides: { CI: '' } };
  let seen;
  const launch = await buildBackend(s, {
    logImpl() {},
    async runPhaseImpl(command, args, options) {
      seen = { command, args, options };
      options.onLine(JSON.stringify({ reason: 'compiler-artifact', target: { name: 'api-server', kind: ['bin'] }, executable: binary }));
    },
  });
  assert.equal(path.basename(seen.command), process.platform === 'win32' ? 'cargo.exe' : 'cargo');
  assert.equal(seen.options.timeoutMs, null);
  assert.ok(seen.args.includes('build'));
  assert.ok(!seen.args.includes('run'));
  assert.equal(seen.options.env.CARGO_BUILD_JOBS, '1');
  assert.equal(seen.options.env.CARGO_MEMORY_BUDGET_ACTIVE, process.env.CARGO_MEMORY_BUDGET_ACTIVE);
  assert.equal(seen.options.cleanup, undefined);
  assert.deepEqual(launch, { command: binary, args: [] });
  assert.deepEqual(s.envOverrides, { CI: '' });
  await assert.rejects(buildBackend(s, {
    logImpl() {}, runPhaseImpl: async () => {},
  }), /without an api-server executable/);
});

test('AC-004 phase reports progress, propagates failure, and times out a real lightweight child', async (t) => {
  const root = fixture(t);
  const logs = [];
  const base = { cwd: root, logFile: path.join(root, 'phase.log'), label: 'fixture', heartbeatMs: 20, logImpl: (line) => logs.push(line) };
  await assert.rejects(runPhase(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { ...base, timeoutMs: 120 }), /timed out/);
  assert.ok(logs.some((line) => line.includes('waiting')));
  await assert.rejects(runPhase(process.execPath, ['-e', 'console.error("fixture error"); process.exit(7)'], base), /fixture error/);
  const result = await runPhase(process.execPath, ['-e', 'console.error("migration fixture"); process.exit(1)'], { ...base, allowFailure: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /migration fixture/);
});

test('AC-005 cancellation kills this phase and its descendant,', async (t) => {
  const root = fixture(t);
  const controller = new AbortController();
  let descendant;
  const source = `const {spawn}=require('node:child_process'); const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'}); console.log(child.pid); setInterval(()=>{},1000);`;
  await assert.rejects(runPhase(process.execPath, ['-e', source], {
    cwd: root, logFile: path.join(root, 'cancel.log'), signal: controller.signal, logImpl() {}, timeoutMs: null,
    onLine(line) { descendant = Number(line); controller.abort(new Error('fixture cancelled')); },
  }), /fixture cancelled/);
  assert.ok(descendant > 0);
  // Linux may briefly retain a dead orphan as a zombie; it must not be running.
  if (process.platform === 'linux' && fs.existsSync(`/proc/${descendant}/stat`)) {
    assert.match(fs.readFileSync(`/proc/${descendant}/stat`, 'utf8'), /\) Z /);
  } else {
    assert.throws(() => process.kill(descendant, 0), { code: 'ESRCH' });
  }
});

test('AC-005 startup lock blocks concurrent invocations and releases cleanly', (t) => {
  const root = fixture(t);
  const release = acquireStartupLock(root);
  assert.throws(() => acquireStartupLock(root), /Another dev-up operation/);
  release();
  acquireStartupLock(root)();
});

test('AC-004 bounded runtime wait stops immediately on cancellation or child exit', async (t) => {
  const s = service(fixture(t));
  fs.writeFileSync(s.pidFile, JSON.stringify({ pid: 42 }));
  let continuing;
  await waitForServicePort(s, async (_host, _port, _timeout, guard) => { continuing = guard(); return false; }, () => false);
  assert.equal(continuing, false);
  const controller = new AbortController();
  controller.abort();
  s.signal = controller.signal;
  await waitForServicePort(s, async (_host, _port, _timeout, guard) => { continuing = guard(); return false; }, () => true);
  assert.equal(continuing, false);
});

test('AC-005 session cancellation cleans only new services and releases signals and lock', async (t) => {
  const { EventEmitter } = require('node:events');
  const { withStartupSession } = require('../phases.js');
  const root = fixture(t);
  const reused = { ...service(root), pidFile: path.join(root, 'reused.json') };
  const created = { ...service(root), startedPid: 42 };
  fs.writeFileSync(reused.pidFile, '{"pid":24}');
  fs.writeFileSync(created.pidFile, '{"pid":42}');
  const signals = new EventEmitter();
  const stopped = [];
  await assert.rejects(withStartupSession([reused, created], root, async () => {
    signals.emit('SIGINT');
    signals.emit('SIGINT');
    assert.equal(created.signal.aborted, true);
  }, { signalSource: signals, stopOwnedProcessImpl: async (pid) => stopped.push(pid) }), /cancelled/);
  assert.deepEqual(stopped, [42]);
  assert.equal(fs.existsSync(reused.pidFile), true);
  assert.equal(fs.existsSync(created.pidFile), false);
  assert.equal(signals.listenerCount('SIGINT'), 0);
  acquireStartupLock(root)();
});

test('AC-005 successful session keeps its service; cleanup failure still attempts other owned services', async (t) => {
  const { EventEmitter } = require('node:events');
  const { withStartupSession } = require('../phases.js');
  const root = fixture(t);
  const services = [1, 2].map((pid) => ({ ...service(root), startedPid: pid, pidFile: path.join(root, `${pid}.json`) }));
  const signals = new EventEmitter();
  await withStartupSession(services, root, async () => {}, {
    signalSource: signals, stopOwnedProcessImpl: () => assert.fail('must keep successful services'),
  });
  const stopped = [];
  await assert.rejects(withStartupSession(services, root, async () => { throw new Error('build failed'); }, {
    signalSource: signals, stopOwnedProcessImpl: async (pid) => { stopped.push(pid); if (pid === 2) throw new Error('cleanup fixture error'); },
  }), /build failed; cleanup failed/);
  assert.deepEqual(stopped, [2, 1]);
  acquireStartupLock(root)();
});

test('AC-004 unbounded build phase emits progress until successful completion', async (t) => {
  const root = fixture(t);
  const logs = [];
  const result = await runPhase(process.execPath, ['-e', 'setTimeout(() => console.log("finished"), 180)'], {
    logFile: path.join(root, 'unbounded.log'), timeoutMs: null, heartbeatMs: 20,
    logImpl: (line) => logs.push(line),
  });
  assert.equal(result.status, 0);
  assert.match(result.stdout, /finished/);
  assert.ok(logs.some((line) => line.includes('timeout=none')));
  assert.ok(logs.some((line) => line.includes('waiting')));
  assert.ok(logs.every((line) => !line.includes('timed out')));
});

test('AC-006 Node startup has no Linux resource-manager dependency', () => {
  for (const file of ['build.js', 'phases.js']) {
    assert.doesNotMatch(fs.readFileSync(path.join(__dirname, '..', file), 'utf8'), /systemctl|systemd-run|rust-build\.slice/);
  }
});
